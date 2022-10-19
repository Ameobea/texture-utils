run:
  cd engine && just build && cd -
  bun run dev --port 9697

build:
  cd engine && just build && cd -
  bun run build

build-wasm:
  cd engine && just build
