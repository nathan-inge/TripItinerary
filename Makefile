.PHONY: dev-backend dev-frontend build build-frontend build-backend test clean deploy install-service

# Local development: run backend and frontend in separate terminals.
dev-backend:
	cd backend && DEV_BYPASS_AUTH=1 cargo run

dev-frontend:
	cd frontend && npm run dev

# Production build: bundles the SPA into the Rust binary.
build:
	cd frontend && npm install && npm run build
	cd backend && cargo build --release

build-frontend:
	cd frontend && npm install && npm run build

build-backend:
	cd backend && cargo build --release

test:
	cd backend && cargo test
	cd frontend && npm test --silent --run || true

clean:
	cd backend && cargo clean
	rm -rf frontend/node_modules frontend/dist

# Deploy: rsync source to CloudDesktop, build remotely, restart service.
# Override REMOTE on the command line: make deploy REMOTE=user@tripit-cd
REMOTE ?= tripit-cd
deploy:
	rsync -avz --exclude target --exclude node_modules --exclude dist \
		./ $(REMOTE):~/tripit-src/
	ssh $(REMOTE) 'cd ~/tripit-src && make build && \
		install -D backend/target/release/tripit ~/.local/bin/tripit && \
		systemctl --user restart tripit'

install-service:
	install -D ops/tripit.service ~/.config/systemd/user/tripit.service
	systemctl --user daemon-reload
	systemctl --user enable --now tripit
	loginctl enable-linger $$USER || true
