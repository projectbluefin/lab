
| 2026-07-20T11:13:28Z |
Phase: Running
```
--- 2026-07-20T11:13:28Z ---
Name:                dakota-iter2-9dg7c
Namespace:           argo
ServiceAccount:      argo
Status:              Running
Conditions:          
 PodRunning          False
Created:             Mon Jul 20 01:11:45 -0400 (6 hours ago)
Started:             Mon Jul 20 01:11:45 -0400 (6 hours ago)
Duration:            6 hours 1 minutes
Progress:            1/5
ResourcesDuration:   8h25m49s*(1 cpu),10h24m30s*(10Gi ephemeral-storage),171h11m48s*(100Mi memory)
Parameters:          
  repo:              https://github.com/projectbluefin/dakota.git
  ref:               testing
  commit-sha:        
  registry:          192.168.1.102:30500
  build-mode:        re
  lock-key:          bst-build-manual

[39mSTEP[0m                       TEMPLATE           PODNAME                                          DURATION  MESSAGE
 [36m●[0m dakota-iter2-9dg7c      build                                                                                                      
 ├─[32m✔[0m detect-build-mode     detect-build-mode  dakota-iter2-9dg7c-detect-build-mode-1869438243  6s                                     
 ├─[36m●[0m build-bluefin         run-bst-step                                                                                               
 │ └─[36m●[0m bst-re(0)           bst-build-re       dakota-iter2-9dg7c-bst-build-re-353507299        6h                                     
 └─[31m✖[0m build-bluefin-nvidia  run-bst-step                                                                                               
   └─[31m✖[0m bst-re              bst-build-re                                                                  No more retries left         
     ├─[31m✖[0m bst-re(0)         bst-build-re       dakota-iter2-9dg7c-bst-build-re-3166185369       35m       main: Error (exit code 255)  
     ├─[31m✖[0m bst-re(1)         bst-build-re       dakota-iter2-9dg7c-bst-build-re-3770326748       15m       main: Error (exit code 255)  
     └─[31m✖[0m bst-re(2)         bst-build-re       dakota-iter2-9dg7c-bst-build-re-1085466423       11m       main: Error (exit code 255)  

Pods:
NAME                                              READY   STATUS      RESTARTS   AGE    IP            NODE    NOMINATED NODE   READINESS GATES
dakota-iter2-9dg7c-bst-build-re-353507299         2/2     Running     0          6h1m   10.42.1.184   exo-0   <none>           <none>
dakota-iter2-9dg7c-detect-build-mode-1869438243   0/2     Completed   0          6h1m   10.42.1.183   exo-0   <none>           <none>

Node CPU/memory top:
NAME    CPU(cores)   CPU(%)   MEMORY(bytes)   MEMORY(%)   
exo-0   116m         0%       3633Mi          5%          
ghost   282m         0%       9793Mi          16%         

BuildBarn worker pods:
NAME           READY   STATUS    RESTARTS   AGE   IP           NODE    NOMINATED NODE   READINESS GATES
worker-wfjw4   2/2     Running   0          21h   10.42.1.96   exo-0   <none>           <none>
worker-zm67q   2/2     Running   0          38h   10.42.0.83   ghost   <none>           <none>
```
