import { test, expect } from '@playwright/test';

test.describe('Animation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('http://localhost:8000/');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000); // Wait for initial image
  });

  test('loads images and plays animation', async ({ page }) => {
    const logs: string[] = [];

    page.on('console', msg => {
      logs.push(msg.text());
      console.log(`Browser: ${msg.text()}`);
    });

    // Set hours to 1 (small amount for faster test)
    await page.fill('#hours', '1');

    // Click load
    await page.click('#load');

    // Wait for loading to complete
    await page.waitForTimeout(8000);

    // Check how many images loaded
    const loadStatus = await page.evaluate(() => {
      return {
        imageCache: (window as any).imageCache?.length || 0,
        currentFrame: (window as any).currentFrame,
        statusText: document.getElementById('status')?.textContent || '',
      };
    });

    console.log('After load:', loadStatus);

    // Click play
    await page.click('#play');
    await page.waitForTimeout(500);

    // Check if animation is playing
    const playStatus = await page.evaluate(() => {
      return {
        isPlaying: (window as any).isPlaying,
        animationInterval: (window as any).animationInterval !== null,
      };
    });

    console.log('After play:', playStatus);

    // Wait and check if frame changes
    const initialFrame = await page.evaluate(() => (window as any).currentFrame);
    await page.waitForTimeout(1000);
    const laterFrame = await page.evaluate(() => (window as any).currentFrame);

    console.log('Frame progression:', { initialFrame, laterFrame });

    expect(loadStatus.imageCache).toBeGreaterThan(0);
    expect(playStatus.isPlaying).toBe(true);
    expect(playStatus.animationInterval).toBe(true);
    expect(laterFrame).not.toBe(initialFrame);
  });
});
