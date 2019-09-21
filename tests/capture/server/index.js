const Koa = require("koa");
const fs = require("fs");
const rawBody = require("raw-body");
const app = new Koa();

const CROSSDOMAIN_XML = `<?xml version="1.0"?>
<!DOCTYPE cross-domain-policy SYSTEM "http://www.adobe.com/xml/dtds/cross-domain-policy.dtd">
<cross-domain-policy>
  <allow-access-from domain="*" />
  <site-control permitted-cross-domain-policies="all" />
</cross-domain-policy>
`;

app.use(async (ctx, next) => {
  const start = Date.now();
  await next();
  const ms = Date.now() - start;
  console.log(`${ctx.method} ${ctx.url} - ${ms}ms`);
});

app.use(async (ctx) => {
  if (ctx.url === "/crossdomain.xml") {
    ctx.body = CROSSDOMAIN_XML;
    ctx.type = "application/xml";
    ctx.status = 200;
  } else if (ctx.method === "POST") {
    try {
      await handleFlashData(ctx);
      ctx.status = 200;
    } catch (err) {
      console.error(err);
      ctx.status = 500;
    }
  } else {
    ctx.status = 404;
  }
});

app.listen(3000, () => console.log("Ready"));

async function handleFlashData(ctx) {
  const body = await rawBody(ctx.req);
  const path = ctx.path;
  const query = ctx.query;
  const width = parseInt(query.width, 10);
  const height = parseInt(query.height, 10);
  checkParameters(path, body, width, height);
  const name = path.substring(1);
  argbToRgba(body);
  const pam = imageDataToPam({width, height, data: body});
  fs.writeFileSync(`${name}.pam`, pam);
}

function argbToRgba(bytes) {
  for (let i = 0; i < bytes.length; i += 4) {
    const a = bytes[i];
    bytes[i] = bytes[i + 1];
    bytes[i + 1] = bytes[i + 2];
    bytes[i + 2] = bytes[i + 3];
    bytes[i + 3] = a;
  }
}

function checkParameters(path, body, width, height) {
  if (!isImageDimension(width)) {
    throw new Error("InvalidWidth");
  }
  if (!isImageDimension(height)) {
    throw new Error("InvalidWidth");
  }
  if (!/\/[a-z]{1,32}/.test(path)) {
    throw new Error("InvalidPath");
  }
  if (width * height * 4 !== body.length) {
    throw new Error("InvalidBody");
  }
}

function isImageDimension(value) {
  return typeof value === "number" && 0 < value && value <= (1 << 16) && Math.floor(value) === value;
}

function imageDataToPam(imageData) {
  const headerParts = [];
  headerParts.push("P7");
  headerParts.push(`WIDTH ${imageData.width.toString(10)}`);
  headerParts.push(`HEIGHT ${imageData.height.toString(10)}`);
  headerParts.push("DEPTH 4");
  headerParts.push("MAXVAL 255");
  headerParts.push("TUPLTYPE RGB_ALPHA");
  headerParts.push("ENDHDR");
  headerParts.push("");
  const header = headerParts.join("\n");

  const headerBuffer = Buffer.from(header, "ascii");
  const dataBuffer = Buffer.from(imageData.data);
  const result = Buffer.allocUnsafe(headerBuffer.length + dataBuffer.length);

  headerBuffer.copy(result);
  dataBuffer.copy(result, headerBuffer.length);

  return result;
}
