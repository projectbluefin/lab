# Skopeo Image Proxy Migration to JSON-RPC FD Passing

The current Skopeo proxy implementation suffers from fundamental protocol limitations that prevent it from scaling to modern container workloads. This document outlines a migration path that addresses these core issues while maintaining backward compatibility.

## Problem Statement

The existing Skopeo proxy protocol has three critical flaws that limit its utility:

First, the SOCK_SEQPACKET transport imposes a 32KB message limit, making it impossible to handle large manifests or metadata that exceed this boundary. This architectural constraint becomes problematic with multi-architecture images and complex annotations.

Second, the custom request/reply format deviates from JSON-RPC 2.0 standards, preventing integration with existing tooling and libraries designed for standard JSON-RPC communication.

Third, the pipe management system using `pipeid` tracking creates unnecessary complexity and potential resource leaks when clients disconnect unexpectedly.

**Current Message Format:**
```go
// Request
{ "method": "MethodName", "args": [arguments] }

// Reply  
{ "success": boolean, "value": JSONValue, "pipeid": number, "error_code": string, "error": string }
```

**File Descriptor Handling:**
- FDs passed directly in ancillary data alongside reply
- Uses `pipeid` for tracking pipe cleanup with `FinishPipe` method
- Supports 1-2 FDs per message (data + optional error pipe)

## Solution Architecture

The migration addresses these limitations through three key architectural changes:

**Transport Evolution:** Moving from SOCK_SEQPACKET to SOCK_STREAM with streaming JSON parsing removes the 32KB limitation while providing reliable message boundaries. This change enables handling of arbitrarily large manifests and complex metadata structures.

**Standards Compliance:** Adopting JSON-RPC 2.0 provides immediate access to existing tooling, debugging capabilities, and client libraries. The structured parameter approach replaces error-prone positional arguments with named parameters that improve type safety.

**Simplified Resource Management:** File descriptor placeholders eliminate the need for explicit pipe tracking, reducing complexity and improving reliability when clients disconnect.

**Protocol Format:**
   ```json
   // New Request Format (GetBlob - was GetRawBlob)
   {
     "jsonrpc": "2.0",
     "method": "GetBlob", 
     "params": {
       "imageID": 12345,
       "digest": "sha256:abc123..."
     },
     "id": 1
   }
   
   // New Reply Format (always two pipes: data + error, indicated by fds field)
   {
     "jsonrpc": "2.0",
     "result": {
       "size": 1024
     },
     "id": 1,
     "fds": 2
   }
   ```

**Resource Management:** The new protocol eliminates pipe lifecycle complexity by using the `fds` field to indicate the number of file descriptors attached. FDs are passed positionally: index 0 is the data pipe, index 1 is the error pipe. Each blob request returns both data and error streams, allowing clients to handle both success and failure cases through the same interface.

## Implementation Approach

**Method Transformation:** Each existing method requires careful translation from positional arguments to structured parameters. The `GetRawBlob` method becomes `GetBlob` with explicit parameter names, making the API self-documenting and reducing integration errors.

**Error Classification:** The existing `proxyErrorCode` system maps cleanly to JSON-RPC 2.0 error codes while preserving the important distinction between retryable network failures and permanent authentication or authorization errors.

**Resource Simplification:** Eliminating the `FinishPipe` mechanism removes a significant source of complexity. File descriptors are passed directly with their usage context, allowing the receiving process to manage them according to its own lifecycle requirements.

## Migration Strategy

**Compatibility Bridge:** The implementation supports both protocols simultaneously during the transition period. Version negotiation occurs during the initial handshake, allowing existing clients to continue functioning while new clients adopt the improved protocol.

**Deployment Phases:** The rollout follows a measured approach where JSON-RPC support is added first, then becomes the default, and finally the legacy protocol is removed after sufficient adoption.

## Technical Benefits

This migration delivers measurable improvements in four key areas:

**Reliability:** SOCK_STREAM transport with streaming JSON parsing eliminates message size limitations that currently prevent handling of large multi-architecture manifests.

**Portability:** SOCK_SEQPACKET availability varies across POSIX systems, particularly macOS. SOCK_STREAM provides universal compatibility.

**Maintainability:** JSON-RPC 2.0 compliance enables use of standard debugging tools, client generators, and validation libraries, reducing the maintenance burden of custom protocol handling.

**Safety:** Named parameters eliminate the class of errors caused by incorrect argument ordering, while structured error responses provide consistent failure handling.

## Protocol Examples

### 1. Initialize Method

**Current Protocol:**
```json
// Request
{ "method": "Initialize", "args": [] }

// Response  
{ "success": true, "value": "0.2.8", "pipeid": 0, "error_code": "", "error": "" }
```

**New JSON-RPC Protocol:**
```json
// Request
{
  "jsonrpc": "2.0",
  "method": "Initialize", 
  "params": {},
  "id": 1
}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "version": "1.0.0"
  },
  "id": 1
}
```

### 2. OpenImage Method

**Current Protocol:**
```json
// Request
{ "method": "OpenImage", "args": ["docker://quay.io/example/image:latest"] }

// Response
{ "success": true, "value": 12345, "pipeid": 0, "error_code": "", "error": "" }
```

**New JSON-RPC Protocol:**
```json
// Request
{
  "jsonrpc": "2.0",
  "method": "OpenImage",
  "params": {
    "imageName": "docker://quay.io/example/image:latest"
  },
  "id": 2
}

// Response
{
  "jsonrpc": "2.0", 
  "result": {
    "imageID": 12345
  },
  "id": 2
}
```

### 3. GetBlob Method (with Two File Descriptors)

**Current Protocol (was GetRawBlob):**
```json
// Request
{ "method": "GetRawBlob", "args": [12345, "sha256:def456..."] }

// Response (with 2 FDs passed via ancillary data)
{ "success": true, "value": 2048, "pipeid": 0, "error_code": "", "error": "" }
```

**New JSON-RPC Protocol:**
```json
// Request
{
  "jsonrpc": "2.0",
  "method": "GetBlob",
  "params": {
    "imageID": 12345,
    "digest": "sha256:def456..."
  },
  "id": 4
}

// Response (with 2 FDs passed positionally: data=0, error=1)  
{
  "jsonrpc": "2.0",
  "result": {
    "size": 2048
  },
  "id": 3,
  "fds": 2
}
```

### 4. Error Handling

**Current Protocol:**
```json
{ "success": false, "value": null, "pipeid": 0, "error_code": "retryable", "error": "Failed to fetch blob: network timeout" }
```

**New JSON-RPC Protocol:**
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32103,
    "message": "Failed to fetch blob: network timeout",
    "data": {
      "errorType": "retryable",
      "category": "network"
    }
  },
  "id": 4
}
```

## Implementation Timeline

The migration follows a structured four-phase approach designed to minimize disruption while ensuring thorough validation:

**Infrastructure Phase (Weeks 1-2):** Establish the JSON-RPC 2.0 foundation with streaming JSON parsing and file descriptor placeholder systems. This phase focuses on the transport layer changes that enable all subsequent improvements.

**Method Migration Phase (Weeks 3-4):** Systematically convert each existing method to the new format. Priority goes to high-usage methods like `GetBlob` and `GetManifest`, with careful attention to maintaining semantic equivalence.

**Integration Phase (Weeks 5-6):** Comprehensive testing across different client scenarios, with particular focus on the compatibility bridge that allows mixed-mode operation during deployment.

**Deployment Phase (Weeks 7-8):** Controlled rollout with monitoring and fallback capabilities, ensuring that any unexpected issues can be quickly addressed without disrupting production systems.

## Expected Outcomes

This migration addresses fundamental limitations that currently prevent Skopeo proxy from scaling to modern container registry requirements. The JSON-RPC 2.0 foundation provides a robust platform for future enhancements while immediately improving reliability and maintainability.

The elimination of message size constraints enables handling of complex multi-architecture images that are becoming standard in cloud-native deployments. Standards compliance reduces integration complexity for tool developers and improves debugging capabilities for operations teams.

Most importantly, the simplified resource management model reduces the likelihood of resource leaks and improves overall system stability, particularly in high-volume scenarios where connection handling becomes critical.

## Risk Management

The migration strategy prioritizes safety through comprehensive backward compatibility and measured deployment. The dual-protocol approach ensures that existing clients continue to function throughout the transition, while extensive testing validates that performance characteristics meet or exceed current benchmarks.

Particular attention is paid to edge cases in file descriptor handling, as this represents the most complex aspect of the protocol change. The testing strategy includes stress testing under high connection loads and resource exhaustion scenarios to ensure robust behavior in production environments.