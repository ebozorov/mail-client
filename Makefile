# Mail client — asosiy buyruqlar
#   make run      — ilovani ochadi (desktop kutubxonalari bo'lsa oynada, bo'lmasa brauzerda)
#   make web      — faqat brauzerda (http://localhost:1420)
#   make desktop  — Tauri desktop oynada
#   make build    — desktop installer (.deb / .AppImage / .rpm) yig'adi
#   make deps     — desktop uchun tizim kutubxonalarini o'rnatadi (sudo)
#   make test     — unit testlar (Rust + TypeScript tekshiruvi)
#   make test-all — + haqiqiy IMAP/SMTP server (Docker, GreenMail), keyring va internet testlari
# To'xtatish: Ctrl+C

SHELL := /bin/bash
GREENMAIL_OPTS := -Dgreenmail.setup.test.all -Dgreenmail.hostname=0.0.0.0 \
	-Dgreenmail.users=me:secret@example.org,bob:secret@example.org -Dgreenmail.users.login=email

.PHONY: run web desktop build deps clean test test-all greenmail-up greenmail-down

HAS_WEBKIT := $(shell pkg-config --exists webkit2gtk-4.1 && echo yes)

run: node_modules
ifeq ($(HAS_WEBKIT),yes)
	@$(MAKE) --no-print-directory desktop
else
	@echo ">> Desktop kutubxonalari topilmadi — brauzerda ochiladi. (Desktop uchun: make deps)"
	@$(MAKE) --no-print-directory web
endif

web: node_modules
	npx vite --open

desktop: node_modules
	npx tauri dev

build: node_modules
	npx tauri build

node_modules: package.json
	npm install
	@touch node_modules

deps:
	sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
		libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev

test: node_modules
	cargo test -p mail-core
	npx tsc -b

test-all: node_modules greenmail-up
	GREENMAIL=1 KEYRING=1 NETWORK=1 cargo test -p mail-core; \
		status=$$?; $(MAKE) --no-print-directory greenmail-down; exit $$status
	npx tsc -b

greenmail-up:
	@docker rm -f mc-greenmail >/dev/null 2>&1 || true
	docker run -d --rm --name mc-greenmail -p 3025:3025 -p 3143:3143 \
		-e GREENMAIL_OPTS='$(GREENMAIL_OPTS)' greenmail/standalone:2.1.2 >/dev/null
	@# Docker portni server tayyor bo'lmasdan ochadi — IMAP salomini ("* OK") kutamiz
	@for i in $$(seq 100); do \
		(exec 3<>/dev/tcp/127.0.0.1/3143 && read -t 2 -r line <&3 && [[ $$line == "* OK"* ]]) 2>/dev/null && exit 0; \
		sleep 0.3; \
	done; echo "GreenMail ishga tushmadi"; exit 1

greenmail-down:
	@docker rm -f mc-greenmail >/dev/null 2>&1 || true

clean:
	rm -rf dist target
