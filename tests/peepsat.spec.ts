import { test, expect } from '@playwright/test';
import { spawn } from 'child_process';
import net from 'net';

async function waitForPort(port: number, host: string, timeoutMs: number) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (await canConnect(port, host)) return;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`Timed out waiting for ${host}:${port}`);
}

function canConnect(port: number, host: string): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.createConnection({ port, host }, () => {
      socket.destroy();
      resolve(true);
    });
    socket.on('error', () => {
      socket.destroy();
      resolve(false);
    });
  });
}

test.describe('PeepSat', () => {
  let server: ReturnType<typeof spawn> | null = null;

  test.beforeAll(async () => {
    server = spawn('cargo', ['run', '--bin', 'server'], {
      cwd: process.cwd(),
      stdio: 'inherit',
    });
    await waitForPort(8000, '127.0.0.1', 15000);
  });

  test.afterAll(async () => {
    if (server) {
      server.kill();
      server = null;
    }
  });

  test('loads wasm app on canvas', async ({ page }) => {
    await page.goto('/');

    const canvas = page.locator('#canvas');
    await expect(canvas).toBeVisible();

     await page.waitForLoadState('networkidle');
     await page.waitForTimeout(2000);
 
      const hasCanvas = await page.evaluate(() => {
        const canvas = document.getElementById('canvas') as HTMLCanvasElement | null;
        return !!canvas && canvas.width > 0 && canvas.height > 0;
      });
 
      expect(hasCanvas).toBeTruthy();
  });
});
