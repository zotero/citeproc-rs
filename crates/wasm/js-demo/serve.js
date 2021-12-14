const esbuild = require('esbuild');
const http = require('http');
const buildOptions = require('./build.js');

// Start esbuild's server on a random local port
esbuild.serve({ servedir: "./build" }, buildOptions).then(result => {
    // The result tells us where esbuild's local server is
    const { host, port } = result

    // Then start a proxy server on port 3001
    http.createServer((req, res) => {
        const options = {
            hostname: host,
            port: port,
            path: req.url,
            method: req.method,
            headers: req.headers,
        }

        // Forward each incoming request to esbuild
        const proxyReq = http.request(options, proxyRes => {
            // If esbuild returns "not found", send a custom 404 page
            if (proxyRes.statusCode === 404) {
                const indexReq = http.request({ ...options, path: "/" }, indexRes => {
                    res.writeHead(indexRes.statusCode ?? 200, indexRes.headers)
                    indexRes.pipe(res, { end: true });
                });
                indexReq.end();
                return;
            }

            // Otherwise, forward the response from esbuild to the client
            res.writeHead(proxyRes.statusCode, proxyRes.headers);
            proxyRes.pipe(res, { end: true });
        });

        // Forward the body of the request to esbuild
        req.pipe(proxyReq, { end: true });
    }).listen(3001);
});
