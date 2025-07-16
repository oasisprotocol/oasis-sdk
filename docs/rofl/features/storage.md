# Persistent Storage

ROFL developers may use Sapphire smart contracts for secure and consistent
storage across all ROFL replicas. This storage however, is not appropriate for
read/write intensive applications. For this reason ROFL has built-in support for
local persistent storage with the following settings:

- Local per-machine storage, not synchronized across other ROFL replicas.
- Fully encrypted on the host machine.
- Preserved during ROFL upgrades and node restarts.

This type of a storage is particularly useful for caching. Docker images defined
in the `compose.yaml` file are automatically stored to persistent storage. This
way they are fetched only the first time a ROFL app is deployed, otherwise
a cached version is considered.

All non-external container volumes will automatically reside in persistent
storage. In the example below, we [define a new volume] called `my-volume` and
make `.ollama` in the home folder persistent. This way we avoid downloading
ollama models each time a machine hosting the ROFL app is restarted:

```yaml title="compose.yaml"
services:
  ollama:
    image: "docker.io/ollama/ollama"
    ports:
      - "11434:11434"
    volumes:
      - my-volume:/root/.ollama
    entrypoint: ["/usr/bin/bash", "-c", "/bin/ollama serve & sleep 5; ollama pull deepseek-r1:1.5b; wait"]

volumes:
  my-volume:
```

[define a new volume]: https://docs.docker.com/reference/compose-file/volumes/
