import React, { Component, Suspense } from 'react';

const AsyncEditor = React.lazy(async () => {
    // Load wasm before making it interactive.
    // Removes failed expectation of immediate response compared to lazily loading it.
    await import('../../pkg');
    return await import('./Editor');
})

class ErrorBoundary extends Component<{}, { hasError: boolean, }> {
    constructor(props: {}) {
        super(props);
        this.state = { hasError: false };
    }

    static getDerivedStateFromError(error: any) {
        // Update state so the next render will show the fallback UI.
        return { hasError: true };
    }

    componentDidCatch(error: any, errorInfo: any) {
        // You can also log the error to an error reporting service
        // logErrorToMyService(error, errorInfo);
    }

    render() {
        if (this.state.hasError) {
            // You can render any custom fallback UI
            return <h1>Something went wrong.</h1>;
        }
        return this.props.children; 
    }
}

const App = () => {
    return (
        <div className="App">
            <header className="App-header">
                <a
                    className="App-link"
                    href="https://github.com/cormacrelf/citeproc-rs"
                    target="_blank"
                    rel="noopener noreferrer"
                >
                    Test driver for <code>citeproc-wasm</code>
                </a>
            </header>
            <ErrorBoundary>
                <Suspense fallback={<div>Loading...</div>}>
                    <AsyncEditor />
                </Suspense>
            </ErrorBoundary>
        </div>
    );
};


export default App;
