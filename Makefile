IMAGE_NAME ?= ray
IMAGE_TAG ?= latest
OUTPUT_DIR ?= $(PWD)

.PHONY: build install docker clean fmt

build:
	cargo build --release

install:
	cargo install --path .

fmt:
	cargo fmt

docker:
	docker build -t $(IMAGE_NAME):$(IMAGE_TAG) .
	docker save $(IMAGE_NAME):$(IMAGE_TAG) | pigz > $(OUTPUT_DIR)/$(IMAGE_NAME)+$(IMAGE_TAG).tar.gz

clean:
	cargo clean
