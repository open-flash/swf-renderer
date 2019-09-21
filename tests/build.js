const fs = require("fs");
const {JsonReader} = require("kryo/readers/json");
const sysPath = require("path");
const {movieToBytes} = require("swf-emitter");
const {CompressionMethod} = require("swf-tree/compression-method");
const {$Movie} = require("swf-tree/movie");

const ROOT = __dirname;

const JSON_READER = new JsonReader();

function main() {
  const dirPath = sysPath.join(ROOT, "movies");
  const srcPath = sysPath.join(dirPath, "src");
  const astPath = sysPath.join(srcPath, "ast.json");
  const astJson = fs.readFileSync(astPath, {encoding: "UTF-8"});
  const movie = $Movie.read(JSON_READER, astJson);
  const movieBytes = movieToBytes(movie, CompressionMethod.None);
  const moviePath = sysPath.join(dirPath, "main.swf");
  fs.writeFileSync(moviePath, movieBytes);
}

main();
