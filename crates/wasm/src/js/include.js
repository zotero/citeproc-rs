export class WasmResult {
    constructor(value) {
        if (value instanceof Error) {
            this.Err = value;
        } else {
            this.Ok = value;
        }
    }
    is_some() {
        if (this.hasOwnProperty("Err")) {
            return false;
        }
        return true;
    }
    is_none() {
        return !this.is_some();
    }
    unwrap() {
        if (this.hasOwnProperty("Err")) {
            throw this.Err;
        } else {
            return this.Ok
        }
    }
    unwrap_or(otherwise) {
        if (this.hasOwnProperty("Err")) {
            return otherwise;
        } else {
            return this.Ok;
        }
    }
    map(func) {
        if (this.hasOwnProperty("Err")) {
            return this;
        } else {
            return new WasmResult(func(this.Ok));
        }
    }
    map_or(otherwise, func) {
        if (this.hasOwnProperty("Err")) {
            return otherwise;
        } else {
            return func(this.Ok);
        }
    }
}

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
    onto.WasmResult = WasmResult;
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
