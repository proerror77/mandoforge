# Build context must contain the pinned upstream ax-task-runner-linux binary
# and ax-coding-agent.py. Final image digest is pinned by the managed profile.
FROM ubuntu@sha256:33ceb71981b602c1a7443a53469e4dba065f7503eab3078a2d7a57a2ab987517
RUN apt-get update && apt-get install -y --no-install-recommends python3 nodejs npm ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g @openai/codex@0.155.1
COPY ax-task-runner-linux /usr/local/bin/ax-task-runner
COPY ax-coding-agent.py /opt/mandoforge/ax-coding-agent.py
ENTRYPOINT ["/usr/local/bin/ax-task-runner"]
