export class CiteprocRsError extends Error {
    constructor(message) {
        super(message);
        this.name = "CiteprocRsError";
    }
}
export class CiteprocRsDriverError extends CiteprocRsError {
    constructor(message, data) {
        super(message);
        this.data = data;
        this.name = "CiteprocRsDriverError";
    }
}
export class CslStyleError extends CiteprocRsError {
    constructor(message, data) {
        super(message);
        this.data = data;
        this.name = "CslStyleError";
    }
}

function doExport(onto) {
    onto.CiteprocRsError = CiteprocRsError;
    onto.CslStyleError = CslStyleError;
    onto.CiteprocRsDriverError = CiteprocRsDriverError;
}

// So there is no way to tell wasm-bindgen to re-export JS items.
// So we have to export them onto a global, if possible, for consumers to use them eg with `instanceof`.
// At the same time, the typescript declarations have to be in a `declare global { }` block.
let env_global;
if (typeof self !== "undefined") {
    env_global = self;
} else if (typeof global !== "undefined") {
    env_global = global;
} else if (typeof window !== "undefined") {
    env_global = window;
}
if (typeof env_global !== "undefined") {
    doExport(env_global)
}
