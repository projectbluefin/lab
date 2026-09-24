# Ubiquitous language

Glossary of canonical terms for the lab.
Terms are added as they are resolved; this file is a glossary only — no implementation details.

| Term | Meaning |
| --- | --- |
| **Factory** | The org-wide OS delivery system: image builds and releases across `projectbluefin/bluefin`, `bluefin-lts`, `dakota`, and `common`, plus their CI pipelines. |
| **Lab** | This repo's internal cluster (Argo Workflows + KubeVirt). Lab results are never presented as factory health. |
| **Console** | KubeStellar Console, the lab's sole private cluster-admin and single-pane UI. It targets the canonical local `ghost` k3s topology; cloud or external multi-cluster layouts are not lab assumptions. |
