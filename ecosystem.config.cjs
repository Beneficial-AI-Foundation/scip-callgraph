/**
 * PM2 ecosystem config for scip-callgraph web viewer.
 *
 * Local (no proxy):        pm2 start ecosystem.config.cjs --only scip-callgraph
 * Remote (reverse proxy):  pm2 start ecosystem.config.cjs --only scip-callgraph-remote
 *
 * For remote, set VITE_ORIGIN before starting if different from default:
 *   VITE_ORIGIN=https://your-domain.com pm2 start ecosystem.config.cjs --only scip-callgraph-remote
 *
 * If port 3001 is taken (e.g. by the reverse proxy), set VITE_PORT in .env or:
 *   VITE_PORT=3002 pm2 start ecosystem.config.cjs --only scip-callgraph-remote
 *
 * The reverse proxy should proxy /graph/ and /graph/* to the Vite port.
 */
module.exports = {
  apps: [
    {
      name: 'scip-callgraph',
      cwd: __dirname + '/web',
      script: 'npx',
      args: 'vite',
      env: { NODE_ENV: 'development' },
      interpreter: 'none',
      watch: false,
    },
    {
      name: 'scip-callgraph-remote',
      cwd: __dirname + '/web',
      script: 'npx',
      args: 'vite',
      env: {
        NODE_ENV: 'development',
        VITE_MODE: 'remote',
        VITE_ORIGIN: process.env.VITE_ORIGIN || 'http://ec2-3-23-60-0.us-east-2.compute.amazonaws.com',
        VITE_PORT: process.env.VITE_PORT || '3001',
      },
      interpreter: 'none',
      watch: false,
    },
  ],
};
