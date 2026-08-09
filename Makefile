SHELL := /bin/bash
.DEFAULT_GOAL := help

CARGO ?= cargo
DEV_ADDRESS ?= 127.0.0.1
DEV_PORT ?= 8000

.PHONY: help config require-config dev watch run build release-check release \
	fmt fmt-check test lint check check-fast audit update deps docs clean

help: ## Показать список доступных команд
	@awk 'BEGIN {FS = ":.*## "; printf "Использование: make <команда>\n\nКоманды:\n"} /^[a-zA-Z_-]+:.*## / {printf "  %-16s %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@printf '\nПараметры dev-сервера: DEV_ADDRESS=%s DEV_PORT=%s\n' "$(DEV_ADDRESS)" "$(DEV_PORT)"

config: ## Создать config.yaml из примера, не перезаписывая существующий
	@if [[ -e config.yaml ]]; then \
		printf '%s\n' 'config.yaml уже существует'; \
	else \
		cp config.example.yaml config.yaml; \
		printf '%s\n' 'Создан config.yaml — замените пример пароля перед запуском'; \
	fi

require-config:
	@if [[ ! -f config.yaml ]]; then \
		printf '%s\n' 'Не найден config.yaml. Выполните make config и настройте пароль.' >&2; \
		exit 1; \
	fi

dev: require-config ## Запустить dev-сервер (DEV_ADDRESS и DEV_PORT можно переопределить)
	ROCKET_ADDRESS="$(DEV_ADDRESS)" ROCKET_PORT="$(DEV_PORT)" $(CARGO) run --locked

watch: require-config ## Перезапускать dev-сервер при изменениях (требует cargo-watch)
	@command -v cargo-watch >/dev/null 2>&1 || { printf '%s\n' 'Установите cargo-watch: cargo install cargo-watch --locked' >&2; exit 1; }
	ROCKET_ADDRESS="$(DEV_ADDRESS)" ROCKET_PORT="$(DEV_PORT)" cargo watch -x 'run --locked'

run: require-config ## Запустить оптимизированную release-версию
	ROCKET_ADDRESS="$(DEV_ADDRESS)" ROCKET_PORT="$(DEV_PORT)" $(CARGO) run --release --locked

build: ## Собрать debug-версию
	$(CARGO) build --locked

release-check: ## Проверить компиляцию release-версии
	$(CARGO) check --release --locked

release: ## Собрать release-бинарник
	$(CARGO) build --release --locked

fmt: ## Отформатировать Rust-код
	$(CARGO) fmt

fmt-check: ## Проверить форматирование без изменения файлов
	$(CARGO) fmt -- --check

test: ## Запустить тесты
	$(CARGO) test --locked

lint: ## Запустить Clippy и считать предупреждения ошибками
	$(CARGO) clippy --all-targets --all-features --locked -- -D warnings

check-fast: fmt-check ## Быстро проверить форматирование и компиляцию
	$(CARGO) check --locked

check: fmt-check test lint ## Выполнить все основные проверки

audit: ## Проверить зависимости через RustSec (требует cargo-audit)
	@command -v cargo-audit >/dev/null 2>&1 || { printf '%s\n' 'Установите cargo-audit: cargo install cargo-audit --locked' >&2; exit 1; }
	$(CARGO) audit

update: ## Обновить совместимые версии в Cargo.lock
	$(CARGO) update

deps: ## Показать дерево прямых зависимостей
	$(CARGO) tree -e normal --depth 1

docs: ## Собрать документацию без зависимостей
	$(CARGO) doc --no-deps --locked

clean: ## Удалить артефакты сборки Cargo
	$(CARGO) clean
