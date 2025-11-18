import { test } from '@playwright/test';

test('debug browser state', async ({ page }) => {
  const errors: string[] = [];
  const logs: string[] = [];

  page.on('console', msg => {
    const text = msg.text();
    logs.push(text);
    console.log(`[${msg.type()}] ${text}`);
  });

  page.on('pageerror', error => {
    errors.push(error.message);
    console.error('[ERROR]', error.message);
  });

  await page.goto('http://localhost:8000/', { waitUntil: 'networkidle' });

  await page.waitForTimeout(3000);

  const state = await page.evaluate(() => {
    return {
      imageCache: (window as any).imageCache,
      currentFrame: (window as any).currentFrame,
      isPlaying: (window as any).isPlaying,
      statusText: document.getElementById('status')?.textContent || '',
      canvasSize: {
        width: (document.getElementById('canvas') as HTMLCanvasElement)?.width,
        height: (document.getElementById('canvas') as HTMLCanvasElement)?.height,
      }
    };
  });

  console.log('\n=== BROWSER STATE ===');
  console.log('Errors:', errors);
  console.log('ImageCache length:', state.imageCache?.length ?? 'undefined');
  console.log('CurrentFrame:', state.currentFrame);
  console.log('IsPlaying:', state.isPlaying);
  console.log('Canvas:', state.canvasSize);
  console.log('Status:\n', state.statusText);
  console.log('All logs:', logs);

  // Take screenshot
  await page.screenshot({ path: 'test-results/browser-state.png', fullPage: true });
  console.log('\nScreenshot saved to test-results/browser-state.png');
});
