const path = require('path');

const { createFrontendConfig } = require('../../eslint.frontend.cjs');

module.exports = createFrontendConfig({
  project: path.join(__dirname, 'tsconfig.eslint.json'),
  ignorePatterns: ['src/routeTree.gen.ts'],
});
