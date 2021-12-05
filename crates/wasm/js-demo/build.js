const esbuild = require('esbuild');
const wasmLoader = require('esbuild-plugin-wasm').default;

const define = {}
for (const k in process.env) {
    if (k === "PUBLIC_URL") {
    }
}

function getEnv(name) {
    define[`process.env.${name}`] = JSON.stringify(process.env[name] || "");
    console.log(name, define['process.env.' + name])
}

getEnv("PUBLIC_URL");
getEnv("HOSTED_SNAPSHOT");

const buildOptions = {
    entryPoints: ['js/index.tsx'],
    outdir: "./build",
    bundle: true,
    minify: true,
    sourcemap: true,
    define,
    plugins: [
        wasmLoader()
    ],
    format: "esm",
    loader: {
        '.csl': 'text',
    },
};

if (require.main === module) {
    (async function () {
        await esbuild.build(buildOptions);
    })();
}

module.exports = buildOptions;

