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

