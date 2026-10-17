const path = require('path');

const { createFrontendConfig } = require('../../eslint.frontend.cjs');

module.exports = createFrontendConfig({
  // tsconfig.json leaves test files out of `tsc --noEmit`; lint them anyway.
  project: path.join(__dirname, 'tsconfig.eslint.json'),
});
