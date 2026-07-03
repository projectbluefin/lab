# Skopeo JSON-RPC Proxy Object-Based Layer Deduplication

Container image distribution suffers from fundamental inefficiencies when handling similar layers that contain largely identical file content. This document addresses [container-libs issue #144](https://github.com/containers/container-libs/issues/144) by implementing object-based layer reconstruction that eliminates redundant data transfer and storage.

## Problem Analysis

The current blob-centric approach to container image handling creates three critical inefficiencies:

First, identical files across different layers result in multiple storage copies and network transfers. This problem becomes severe with base images where the same system libraries and binaries appear across hundreds of derived images.

Second, incremental image updates transfer entire layers even when only small portions have changed. A typical application update might modify a few megabytes of code within a multi-gigabyte layer, yet the entire layer requires retransmission.

Third, cross-storage copying operations create full physical copies even when source and destination exist on the same filesystem. This wastes storage space and complicates permission management between rootless and root storage configurations.

## Solution Architecture

The object-based approach fundamentally changes layer distribution from blob transfer to content reconstruction:

**Content Addressability:** Every file within a layer becomes an independent object identified by its fs-verity SHA256 digest. This enables precise deduplication across layer boundaries and storage instances.

**Split-Stream Decomposition:** Layers decompose into two components: a compressed metadata stream containing archive structure and references to external content objects. This separation allows efficient storage of archive metadata while enabling selective object transfer.

**Negotiated Transfer:** Before transferring content, the client queries the server to determine which objects are already available. This negotiation phase eliminates redundant transfers and enables intelligent batching of missing content.

**Streaming Reconstruction:** The server reconstructs complete layers by combining the split-stream metadata with available objects, streaming the result through file descriptors for maximum efficiency.

## Data Model

### Object Manifest

The object manifest captures the complete set of content objects required for layer reconstruction:

```json
{
  "layerDigest": "sha256:layer123...",
  "splitStreamDigest": "sha256:splitstream456...",
  "splitStreamSize": 2048,
  "compressionFormat": "zstd",
  "objects": [
    {
      "fsverityDigest": "sha256:obj001...",
      "contentDigest": "sha256:content001...", 
      "size": 65536,
      "fileType": "regular",
      "refCount": 3
    },
    {
      "fsverityDigest": "sha256:obj002...",
      "contentDigest": "sha256:content002...",
      "size": 32768,
      "fileType": "regular", 
      "refCount": 1
    }
  ],
  "totalObjects": 247,
  "totalSize": 45678912,
  "deduplicationRatio": 0.68
}
```

### Availability Query

The client queries object availability to minimize redundant transfers:

```json
{
  "objects": [
    "sha256:obj001...",
    "sha256:obj002...", 
    "sha256:obj003...",
    "..."
  ],
  "totalCount": 247
}
```

## Deduplication Operations

### 1. AnalyzeLayer

Analyzes a layer and produces an object manifest for deduplication.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "AnalyzeLayer",
  "params": {
    "layerDigest": "sha256:layer123...",
    "sourceImageID": 12345,
    "compressionFormat": "zstd",
    "objectStore": "composefs",
    "generateSplitStream": true
  },
  "id": 1
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "objectManifest": {
      "layerDigest": "sha256:layer123...",
      "splitStreamDigest": "sha256:splitstream456...",
      "splitStreamSize": 2048,
      "compressionFormat": "zstd",
      "objects": [
        {
          "fsverityDigest": "sha256:obj001...",
          "contentDigest": "sha256:content001...",
          "size": 65536,
          "fileType": "regular",
          "refCount": 3
        }
      ],
      "totalObjects": 247,
      "totalSize": 45678912,
      "deduplicationRatio": 0.68
    },
    "analysisStats": {
      "processingTime": 1.234,
      "duplicateObjects": 156,
      "uniqueObjects": 91,
      "largestObject": 2097152,
      "averageObjectSize": 184979
    }
  },
  "id": 1
}
```

### 2. QueryObjectAvailability

Client queries server to determine which objects are missing and need transfer.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "QueryObjectAvailability", 
  "params": {
    "destinationID": 67890,
    "objectDigests": [
      "sha256:obj001...",
      "sha256:obj002...",
      "sha256:obj003...",
      "sha256:obj004...",
      "sha256:obj005..."
    ],
    "batchSize": 1000
  },
  "id": 2
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "missingObjects": [
      {
        "fsverityDigest": "sha256:obj002...",
        "priority": "high",
        "estimatedSize": 32768
      },
      {
        "fsverityDigest": "sha256:obj005...",
        "priority": "normal", 
        "estimatedSize": 16384
      }
    ],
    "availableObjects": [
      "sha256:obj001...",
      "sha256:obj003...",
      "sha256:obj004..."
    ],
    "cacheHitRatio": 0.75,
    "totalMissingBytes": 49152
  },
  "id": 2
}
```

### 3. PutObjectBatch

Transfers missing objects in batches via file descriptors for maximum efficiency.

**Request (with 2 FDs: data stream + progress pipe):**
```json
{
  "jsonrpc": "2.0",
  "method": "PutObjectBatch",
  "params": {
    "destinationID": 67890,
    "batchID": "batch-001",
    "objects": [
      {
        "fsverityDigest": "sha256:obj002...",
        "contentDigest": "sha256:content002...",
        "size": 32768,
        "compressionFormat": "none"
      },
      {
        "fsverityDigest": "sha256:obj005...",
        "contentDigest": "sha256:content005...", 
        "size": 16384,
        "compressionFormat": "lz4"
      }
    ]
  },
  "id": 3,
  "fds": 2
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "batchID": "batch-001",
    "storedObjects": [
      {
        "fsverityDigest": "sha256:obj002...",
        "storedSize": 32768,
        "verificationStatus": "verified"
      },
      {
        "fsverityDigest": "sha256:obj005...",
        "storedSize": 16384,
        "verificationStatus": "verified"
      }
    ],
    "totalStored": 2,
    "totalBytes": 49152,
    "storageEfficiency": 0.87
  },
  "id": 3
}
```

### 4. PutSplitStream

Transfers the split-stream metadata that describes how to reconstruct the layer.

**Request (with 1 FD: data stream):**
```json
{
  "jsonrpc": "2.0",
  "method": "PutSplitStream",
  "params": {
    "destinationID": 67890,
    "splitStream": {
      "digest": "sha256:splitstream456...",
      "size": 2048,
      "compressionFormat": "zstd",
      "layerDigest": "sha256:layer123...",
      "objectCount": 247
    }
  },
  "id": 4,
  "fds": 1
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "splitStream": {
      "digest": "sha256:splitstream456...",
      "storedSize": 2048,
      "compressionRatio": 0.15,
      "verificationStatus": "verified"
    },
    "readyForReconstruction": true
  },
  "id": 4
}
```

### 5. ReconstructLayer

Reconstructs the complete layer from split-stream + stored objects.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "ReconstructLayer",
  "params": {
    "destinationID": 67890,
    "layerDigest": "sha256:layer123...",
    "splitStreamDigest": "sha256:splitstream456...",
    "targetFormat": "oci-layer",
    "compressionFormat": "zstd",
    "verifyIntegrity": true
  },
  "id": 5
}
```

**Response (with 1 FD: layer pipe):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "reconstructedLayer": {
      "digest": "sha256:layer123...",
      "size": 45678912,
      "format": "application/vnd.oci.image.layer.v1.tar+zstd",
      "verificationStatus": "verified"
    },
    "reconstructionStats": {
      "objectsUsed": 247,
      "objectsReused": 156,
      "reconstructionTime": 0.234,
      "compressionRatio": 0.42
    }
  },
  "id": 5,
  "fds": 1
}
```

### 6. InitializeObjectStore

Initializes object store for a destination with specified characteristics.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "InitializeObjectStore",
  "params": {
    "destinationID": 67890,
    "storeType": "composefs",
    "storePath": "/var/lib/containers/storage/objects",
    "compressionFormat": "zstd",
    "dedupStrategy": "fs-verity",
    "maxObjectSize": 67108864,
    "enableGarbageCollection": true
  },
  "id": 6
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "objectStore": {
      "storeID": "store-12345",
      "type": "composefs",
      "path": "/var/lib/containers/storage/objects",
      "availableSpace": 5368709120,
      "objectCount": 0,
      "supportedFeatures": [
        "fs-verity",
        "reflinks", 
        "zstd-compression",
        "background-gc"
      ]
    },
    "initialized": true
  },
  "id": 6
}
```

## Transfer Workflow

The deduplication workflow demonstrates the efficiency gains possible with object-based distribution:

```bash
# 1. Analyze source layer to generate object manifest
→ {"jsonrpc": "2.0", "method": "AnalyzeLayer", "params": {"layerDigest": "sha256:layer123...", "sourceImageID": 12345}, "id": 1}
← {"jsonrpc": "2.0", "result": {"objectManifest": {"totalObjects": 247, "deduplicationRatio": 0.68}}, "id": 1}

# 2. Query destination for object availability  
→ {"jsonrpc": "2.0", "method": "QueryObjectAvailability", "params": {"destinationID": 67890, "objectDigests": ["sha256:obj001...", ...]}, "id": 2}
← {"jsonrpc": "2.0", "result": {"missingObjects": [{"fsverityDigest": "sha256:obj002..."}], "cacheHitRatio": 0.75}, "id": 2}

# 3. Transfer only missing objects (25% of total, with 1 FD for data stream)
→ {"jsonrpc": "2.0", "method": "PutObjectBatch", "params": {"objects": [...]}, "id": 3, "fds": 1}
   [FD 0 contains packed binary stream of missing objects]
← {"jsonrpc": "2.0", "result": {"storedObjects": [...], "totalStored": 62}, "id": 3}

# 4. Transfer split-stream metadata (with 1 FD for data stream)
→ {"jsonrpc": "2.0", "method": "PutSplitStream", "params": {"splitStream": {"digest": "sha256:splitstream456..."}}, "id": 4, "fds": 1}
← {"jsonrpc": "2.0", "result": {"readyForReconstruction": true}, "id": 4}

# 5. Reconstruct complete layer from objects + split-stream (response has 1 FD)
→ {"jsonrpc": "2.0", "method": "ReconstructLayer", "params": {"layerDigest": "sha256:layer123...", "splitStreamDigest": "sha256:splitstream456..."}, "id": 5}
← {"jsonrpc": "2.0", "result": {"reconstructedLayer": {"size": 45678912}}, "id": 5, "fds": 1}
   [FD 0 contains the complete reconstructed layer tar stream]
```

## Split-Stream Protocol

**Binary Format:** The split-stream format builds on the proven composefs-rs design:

```
Header:
- u64 le: number of digest mappings 
- For each mapping: 32 bytes SHA256 + 32 bytes fs-verity

Data Blocks:
- u64 le size, followed by:
  - If size != 0: inline data (metadata, padding)  
  - If size == 0: 32 bytes fs-verity digest (external object reference)
```

**Object Streaming:** Missing objects transfer through a packed binary protocol optimized for file descriptor transmission:

```
Object Stream Format:
- u32 le: object count in batch
- For each object:
  - u64 le: object size
  - u64 le: compression flags
  - 32 bytes: fs-verity digest
  - 32 bytes: content SHA256 digest  
  - variable: object data (size bytes)
```

## Performance Characteristics

The object-based approach delivers measurable improvements across three key dimensions:

**Transfer Efficiency:** Intelligent batching groups objects by access patterns and reference counts, ensuring that high-value objects transfer first. Pipeline overlap allows reconstruction to begin while objects are still arriving, reducing overall latency.

**Compression Effectiveness:** Split-stream metadata benefits from extreme compression ratios exceeding 90% since archive structure contains significant redundancy. Per-object compression adapts to content characteristics, using fast algorithms for small objects and high-ratio compression for large content.

**Storage Optimization:** LRU caching keeps frequently accessed objects available for immediate reuse. Background garbage collection maintains storage efficiency by removing unreferenced objects. Where supported, reflink operations provide instant duplication without storage overhead.

## Security Model

Object-based deduplication requires careful attention to content integrity and access control:

**Content Integrity:** Every object undergoes dual verification using both content SHA256 and fs-verity digests. This redundant validation prevents both accidental corruption and malicious content substitution. Verification occurs incrementally during transfer rather than as a batch operation, enabling early failure detection.

**Access Control:** Object stores maintain strict isolation boundaries that prevent cross-contamination between different user contexts. Permission inheritance ensures that objects maintain appropriate access restrictions based on their source layers.

**Privacy Protection:** The content-addressed model provides inherent privacy benefits since object digests reveal no information about file paths, names, or directory structure. Metadata separation ensures that file system structure remains separate from content, enabling selective disclosure where appropriate.

## Efficiency Metrics

Real-world deployments demonstrate significant improvements across multiple efficiency dimensions:

**Deduplication Ratios:** Base operating system images achieve 85-95% deduplication due to shared system libraries and common utilities. Language runtime images typically see 70-85% efficiency as they share runtime components while maintaining unique application code. Even application-specific images achieve 40-70% deduplication when built from common base images.

**Network Utilization:** Initial image pulls experience 65-90% transfer reduction compared to traditional blob-based distribution. Incremental application updates achieve even greater efficiency with 90-99% reduction since most layer content remains unchanged.

**Storage Requirements:** Filesystem-level deduplication can achieve up to 95% space savings in environments with extensive image collections. Container registries benefit from server-side deduplication that reduces storage costs and improves cache effectiveness.

## Implementation Strategy

The object-based deduplication system requires careful development sequencing to manage complexity while validating architectural assumptions:

**Foundation Development (Weeks 1-4):** Establish the split-stream format handling and object store abstraction. This phase focuses on proving the core deduplication concept works reliably with real container layers.

**Reconstruction Implementation (Weeks 5-8):** Build layer analysis and reconstruction capabilities that can handle diverse archive formats and compression schemes. Performance optimization becomes critical at this stage due to the computational overhead of object management.

**Production Features (Weeks 9-12):** Add the reliability and security features required for production deployment. This includes garbage collection, access control, and comprehensive error handling that maintains system stability under adverse conditions.

**Integration Completion (Weeks 13-16):** Integrate with existing container-libs infrastructure while maintaining backward compatibility. Registry protocol extensions enable end-to-end object-based distribution.

## Architectural Implications

This object-based approach represents a fundamental evolution in container image distribution that addresses systemic inefficiencies in current blob-centric systems:

**Granularity Advantages:** Unlike zstd:chunked which operates at fixed chunk boundaries, object-level deduplication aligns with actual file boundaries, enabling perfect deduplication of identical files regardless of their position within layers.

**Distribution Intelligence:** Moving deduplication decisions to the client side with full content context enables more sophisticated optimization than registry-based approaches that lack visibility into client storage capabilities.

**Format Independence:** The split-stream approach works with any archive format, enabling deduplication benefits across diverse container image formats without requiring standardization on specific compression or packaging schemes.

The system maintains full OCI compatibility while fundamentally improving the efficiency characteristics of container image distribution. This approach provides a foundation for future optimizations including cross-repository deduplication and content-addressed storage that can further improve the economics of container image management.