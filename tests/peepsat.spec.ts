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

  test('loads and displays satellite image', async ({ page }) => {
    const errors: string[] = [];
    const logs: string[] = [];

    // Capture console messages
    page.on('console', msg => {
      const text = msg.text();
      logs.push(`${msg.type()}: ${text}`);
      console.log(`Browser ${msg.type()}: ${text}`);
    });

    // Capture errors
    page.on('pageerror', error => {
      errors.push(error.message);
      console.error('Browser error:', error.message);
    });

    await page.goto('http://localhost:8000/');

    // Wait for the page to load
    await page.waitForLoadState('networkidle');

    // Check for JavaScript errors
    if (errors.length > 0) {
      console.error('JavaScript errors found:', errors);
    }

    // Check if canvas exists and has dimensions
    const canvas = page.locator('#canvas');
    await expect(canvas).toBeVisible();

    const canvasInfo = await page.evaluate(() => {
      const canvas = document.getElementById('canvas') as HTMLCanvasElement | null;
      const status = document.getElementById('status');
      return {
        canvasExists: !!canvas,
        width: canvas?.width || 0,
        height: canvas?.height || 0,
        statusText: status?.textContent || '',
      };
    });

    console.log('Canvas info:', canvasInfo);
    console.log('Status log:', canvasInfo.statusText);

    // Wait a bit for image to load
    await page.waitForTimeout(5000);

    // Check if image was drawn on canvas
    const hasImageData = await page.evaluate(() => {
      const canvas = document.getElementById('canvas') as HTMLCanvasElement;
      if (!canvas) return false;
      const ctx = canvas.getContext('2d');
      if (!ctx) return false;
      const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
      // Check if there's any non-black pixel
      for (let i = 0; i < imageData.data.length; i += 4) {
        if (imageData.data[i] !== 0 || imageData.data[i+1] !== 0 || imageData.data[i+2] !== 0) {
          return true;
        }
      }
      return false;
    });

    console.log('Has image data:', hasImageData);

    // Get final status
    const finalStatus = await page.evaluate(() => {
      return {
        statusText: document.getElementById('status')?.textContent || '',
        imageCache: (window as any).imageCache?.length || 0,
        currentFrame: (window as any).currentFrame,
      };
    });

    console.log('Final status:', finalStatus);

    expect(errors.length).toBe(0);
    expect(canvasInfo.width).toBeGreaterThan(0);
    expect(canvasInfo.height).toBeGreaterThan(0);
    expect(hasImageData).toBeTruthy();
  });
});
