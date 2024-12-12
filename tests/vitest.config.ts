import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['suites/**/*.ts', 'suites/**/*.spec.ts'],
    testTimeout: 600_000,
  },
});

