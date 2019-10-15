async function main() {
  const {createRenderer, destroyRenderer} = await import("../pkg/index.js");
  const renderer = createRenderer();
  renderer.render();
  destroyRenderer(renderer);
}

main().catch(console.error);
