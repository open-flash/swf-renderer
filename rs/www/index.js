async function main() {
  const {createRenderer, destroyRenderer} = await import("../pkg/index.js");
  const canvas = document.getElementById("canvas");
  const renderer = createRenderer(canvas);
  renderer.render();
  destroyRenderer(renderer);
}

main().catch(console.error);
