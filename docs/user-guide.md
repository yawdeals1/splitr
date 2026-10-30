# Splitr user guide

Splitr turns one PDF into several smaller PDFs, such as one file per chapter, per invoice or per student. You always see the proposed parts and can change them before any file is saved. Your original PDF is never changed.

New to Splitr? Install it from the [download page](download.md), then try [your first split](#your-first-split).

## The window

- **Left:** your PDF. Before a file is open, this is the drop area. After a split, the parts to review appear here.
- **Right:** split options: the **Range mode** switch (Custom, Fixed, Smart), the options for that mode, and the **Split** button.

Everything can be done with the keyboard. **Tab** moves between controls, **Space** or **Enter** presses a button, and the arrow keys change the range mode. The page thumbnails count as one stop for **Tab**: use the arrow keys, **Home** and **End** to move between pages, and **Enter** to open the larger view.

## Your first split

This splits a PDF into parts of 10 pages each.

1. **Open a PDF.** Drag it onto the window, or click **Choose file** (shortcut: **Ctrl+O**, or **Cmd+O** on a Mac). Splitr shows the file name, number of pages and size.
2. **Pick a mode.** Under **Range mode**, choose **Fixed**.
3. **Set the size.** In **Split every N pages**, type `10`.
4. **Split.** Click **Split**. The review screen lists the parts: pages 1–10, 11–20 and so on.
5. **Check the parts.** Give a part a title or change its pages if you need to (see [Review and fix the parts](#review-and-fix-the-parts)).
6. **Export.** Click **Export**, choose a folder, and Splitr saves one PDF per part. Click **Open folder** to see them.

## Opening a file

- Open one PDF at a time. If you drop several files, or a file that isn't a PDF, Splitr tells you and nothing is opened.
- **Password-protected PDFs:** Splitr asks for the password. It's only used to read the file and is not saved.
- If a file is damaged or not supported, Splitr says so and stays open, so you can try another file.
- To switch files, click **Open another** or drop a new PDF on the window.

## The three range modes

### Custom: type the pages yourself

Use this when you already know the page numbers. In **Page ranges**, type ranges separated by commas:

| You type | You get |
| --- | --- |
| `1-14, 15-30, 31` | Three files: pages 1–14, pages 15–30 and page 31 |
| `3-5, 9-12` | Two files. Pages 1–2, 6–8 and everything after 12 are left out |

Ranges must be in order and must not overlap. Splitr tells you what's wrong if they don't fit, for example "Range 40-45 is outside pages 1 to 38".

### Fixed: equal-sized parts

Use this to cut a long file into chunks. **Split every N pages** makes parts of N pages; the last part holds whatever is left.

### Smart: split by meaning

Use this when the split points depend on what's on the pages.

1. Choose a **Document category**, such as *Invoices and billing documents* or *Books and general reading documents*.
2. Choose a **Split preset** from that category, such as *Split by invoice number* or *Split by chapter*.
3. Click **Split**.

Presets work in one of two ways.

Some presets use a fast method that works offline:

- **Bookmarks:** one part per top-level bookmark (table of contents entry) in the PDF, for example *Split by chapter*.
- **Text pattern:** a new part wherever a page matches, for example "Invoice No. 1042" for *Split by invoice number*.
- **Blank pages:** a new part at each blank separator page, for *Split at blank pages*.

If the PDF has no bookmarks, or nothing on its pages matches, and you've turned on AI, the preset asks the AI instead. If AI is off, Splitr says it found no split points. When the offline method does find split points, the AI isn't used, so check the parts on the review screen.

Other presets need the AI from the start, because the split depends on what the pages mean, for example *Split by vendor*, *Split exams by student* or *Split by patient*. These only work when AI is turned on.

**Custom prompt.** If no preset fits, choose **Custom prompt** in **Split preset** and describe the split in your own words, for example *Split whenever a new client name appears*. To reuse it later, click **Save as preset**, give it a name and a category, and click **Save preset**. Your presets appear in the lists with the built-in ones. Select one to **Edit preset** or **Delete preset** it.

## Smart mode and AI

AI is optional and off until you set it up. To use it:

1. Under **AI** in the panel, paste an API key for an AI service you have an account with, such as Claude, OpenAI or Gemini, into **Add an API key** and click **Next**. Splitr works out which service the key belongs to from how it starts, without sending it anywhere. If it can't tell, or got it wrong, pick the service yourself, then click **Save key**. Splitr then checks the key with one call to that service only, and shows **Connected** or the exact error. You can save keys for several services and switch between them with **AI service**.
   Or use your ChatGPT plan instead of a key: choose **ChatGPT account** as the **AI service** and click **Connect ChatGPT**. ChatGPT's sign-in page opens in your browser; sign in there and allow Splitr to use your plan. Splitr never sees your password. The panel then shows the connected account and its plan. Each split counts against your plan's usage, which you can limit in ChatGPT under **Settings > Usage**. **Disconnect** ends the connection at OpenAI and removes it from your computer.
   Or run the AI on your own computer, with no account and no internet: install a model server such as [Ollama](https://ollama.com) or [LM Studio](https://lmstudio.ai), download a model in it and start its server. Then choose **Local model (offline)** as the **AI service**. **Address** is Ollama's (`http://localhost:11434/v1`) unless you change it; for LM Studio enter `http://localhost:1234/v1`. Click **Show available models** and pick one for **Model**. The address must be on your computer, so page text never leaves it; for the same reason Splitr doesn't use Ollama's cloud models (names ending in `-cloud`). A local model is slower than an online service, and small models get split points wrong more often.
2. The first time you run an AI split with an online service, Splitr explains what it sends and asks for your consent. A local model sends nothing off your computer, so it doesn't ask.

What is sent: the first and last lines of each page and the page numbers, along with your instruction. The whole file is never sent. Rule-based splits (Custom, Fixed, bookmarks, patterns and blank pages) never use the internet. You can turn AI off at any time in **Settings**.

AI can get split points wrong, so always check the parts on the review screen before you export.

## Review and fix the parts

After **Split**, every proposed part is listed with its title, first page, last page and page count.

| To do this | Do this |
| --- | --- |
| Rename a part | Type in its **Title** box. The title can be used in the file name |
| Move where a part starts or ends | Change its **Start** or **End**. The neighbouring part moves with it, so no pages are lost |
| Add a split | Click **Add split** on the part, type the page that should start the new part, and press **Enter** (or **Escape** to cancel) |
| Remove a split | Click **Join next** to join a part with the one after it |
| Leave a part out | Click **Delete**. Its pages won't be exported, and the screen shows how many pages are left out |
| Start over | Click **Back**, change the options, and click **Split** again |

Splitr checks the list after every change. If something doesn't fit, for example a part that ends before it starts, a message explains the problem and **Export** stays off until it's fixed.

## Export

1. Under **File names** on the review screen, set the template. These words are replaced for each part:

   | Token | Becomes |
   | --- | --- |
   | `{index}` | The part number: 01, 02, 03 and so on (001, 002... when there are 100 parts or more) |
   | `{title}` | The part's title |
   | `{original}` | The name of the PDF you opened |

   For example, `{original} - {index} - {title}` gives `Textbook - 03 - Memory Hierarchy.pdf`. A part without a title drops the title, the separator before it and any brackets around it, so you get `Textbook - 04.pdf`. Characters that aren't allowed in file names are replaced, and very long names are shortened. If two parts would get the same name, Splitr adds a number such as " (2)" to the second one.

   The list under the template shows the names your parts will get, and changes as you edit the template or the titles.
2. Optionally, tick **Bundle into one ZIP file**. The ZIP is named after the PDF you opened.
3. Click **Export** and choose the folder to save into. Splitr remembers the folder and the template for next time. If the folder already has a file with one of the names, Splitr adds a number rather than replace it, and says so.
4. When it's done, click **Open folder** to see your files.

If an export fails part way, for example because the disk is full, Splitr leaves no half-written files behind.

## Settings

- **Theme:** Light, Dark, or System to follow your computer. It's at the bottom of the split options on the right.
- **AI:** turn Smart mode's AI on or off, and add or remove API keys. Only the last 4 characters of a saved key are shown. **AI service** picks what Smart mode uses: your ChatGPT account (the default), a Claude, OpenAI or Gemini API key (Gemini keys come from Google AI Studio), an API key for a platform that works like OpenAI's (OpenRouter, NVIDIA, Groq, xAI, DeepSeek, Mistral or Together AI), an **OpenCode Go subscription** key (from the OpenCode console), a **Local model (offline)** on your own computer (see [Smart mode and AI](#smart-mode-and-ai)), or **Other OpenAI-compatible platform**, where you also enter its address. Addresses must start with `https://`, except for a model running on your own computer (`http://localhost`). **Model** sets the model for that service; leave it blank for the default (GPT-6.1 Sol for ChatGPT and OpenAI, Claude Opus 5.5 for Claude, Gemini 4 Argon for Gemini, DeepSeek Flash for DeepSeek). For OpenCode Go and the OpenAI-compatible platforms, click **Show available models** to pick from the platform's list. OpenCode Go limits use per 5 hours, week and month; when a limit is reached, Splitr says when it resets.
- **Blank page sensitivity:** how much may be on a page (such as a page number) for it to still count as blank.

## Updates

Each time it starts, Splitr checks GitHub for a newer version. This check is the only time Splitr goes online without an AI split. If a newer version exists, a notice under the split options names it. Click **Install and restart** to download it and restart, or **Later** to keep working. Splitr checks each update's signature before installing it, so only updates published by Splitr's developer install.

## Troubleshooting

| You see | What to do |
| --- | --- |
| "This PDF is password protected" | Enter the file's password when asked |
| "This PDF has no bookmarks" | Try a Text pattern or Custom prompt preset, or use Custom or Fixed mode |
| "No split points were found" | The preset found nothing to split on. If AI is off, turn it on so the preset can ask the AI. Otherwise try another preset, a custom prompt, or Custom mode |
| No text found on scanned pages | Scanned pages are images with no text, so Smart mode can't read them yet, and *Split at blank pages* would treat every page as blank. Use Custom or Fixed mode. Text recognition (OCR) is planned |
| "Splitr couldn't reach Local model on this computer" | Start your model server (open Ollama or LM Studio and start its server), and check that **Address** under **AI** matches it |
| A warning when installing or first opening Splitr | See [Why your computer warns you](download.md#why-your-computer-warns-you) |
