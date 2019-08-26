# citeproc-js-runner

## Install

```sh
yarn
yarn link
```

## Functionality 

### 1. Run single YAML-based tests

...using `citeproc-test-runner` internally, to help determine what the output 
should be. Useful for writing a new test.

```sh
cd ../crates/citeproc/tests/data/humans

citeproc-js-runner run some_Test.yml
```


### 2. Convert the txt-based tests to YAML

```sh
citeproc-js-runner to-yml some_TestInTheTestSuite.txt
```

It's best-effort and not standardised (yet).
