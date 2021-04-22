// Modified from https://github.com/alexeyraspopov/jest-webdriver/blob/129c376f10a8ed28d9dcc33f325ee6720641857a/packages/jest-environment-webdriver/modules/WebDriverEnvironment.js

const NodeEnvironment = require('jest-environment-node');
const { Builder, By, until } = require('selenium-webdriver');

class WebDriverEnvironment extends NodeEnvironment {
  constructor(config) {
    super(config);
    const options = config.testEnvironmentOptions || {};
    this.browserName = options.browser || 'chrome';
    this.seleniumAddress = options.seleniumAddress || null;
    this.options = options.options || {};
  }

  async setup() {
    await super.setup();
    let builder = new Builder();
    if (this.seleniumAddress) {
      builder = builder.usingServer(this.seleniumAddress);
    }
    builder = await builder.forBrowser(this.browserName);
    builder.setFirefoxOptions(this.options)
    builder.setChromeOptions(this.options)
    builder.setSafariOptions(this.options)

    let driver = builder.build();

    this.driver = driver;

    this.global.By = By;
    this.global.browser = driver;
    this.global.element = locator => driver.findElement(locator);
    this.global.element.all = locator => driver.findElements(locator);
    this.global.until = until;
  }

  async teardown() {
    if (this.driver) {
      // https://github.com/alexeyraspopov/jest-webdriver/issues/8
      try {
        await this.driver.close();
      } catch (error) { }

      // https://github.com/mozilla/geckodriver/issues/1151
      try {
        await this.driver.quit();
      } catch (error) { }
    }

    await super.teardown();
  }
}

module.exports = WebDriverEnvironment;
