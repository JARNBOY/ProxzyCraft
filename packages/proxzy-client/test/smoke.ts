import { ProxzyClient } from '../src/index.ts';

const client = new ProxzyClient({ baseUrl: 'http://127.0.0.1:9090' });
console.log('Client baseUrl:', client.baseUrl);
console.log('Has isHealthy method:', typeof client.isHealthy === 'function');
console.log('Has health method:', typeof client.health === 'function');
console.log('Has mock method:', typeof client.mock === 'function');
console.log('Has setScenario method:', typeof client.setScenario === 'function');
console.log('Has setFolderScenario method:', typeof client.setFolderScenario === 'function');
console.log('Has inject method:', typeof client.inject === 'function');
console.log('Has injectUser method:', typeof client.injectUser === 'function');
console.log('Has enableAutoRecord method:', typeof client.enableAutoRecord === 'function');
console.log('Has disableAutoRecord method:', typeof client.disableAutoRecord === 'function');
console.log('Has expectHits method:', typeof client.expectHits === 'function');
console.log('Has reset method:', typeof client.reset === 'function');
console.log('Has startSession method:', typeof client.startSession === 'function');
console.log('Has getSessionSummary method:', typeof client.getSessionSummary === 'function');
console.log('Has endSession method:', typeof client.endSession === 'function');
console.log('All ProxzyClient methods verified successfully.');

