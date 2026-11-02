// The download shared by the fetch scripts.

/** Downloads a file, retrying up to four times on network errors. */
export async function download(url) {
  for (let attempt = 1; ; attempt++) {
    console.log(`Downloading ${url}`);
    try {
      const response = await fetch(url);
      if (!response.ok) throw new Error(`Download failed: HTTP ${response.status} for ${url}`);
      return Buffer.from(await response.arrayBuffer());
    } catch (error) {
      if (attempt === 5) throw error;
      console.log(`${error.message}, retrying`);
      await new Promise((resolve) => setTimeout(resolve, 2000 * attempt));
    }
  }
}
