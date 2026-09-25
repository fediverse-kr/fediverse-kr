# Demonstration imagery

`neighborhood.png` was generated with the built-in image generation tool on 2026-09-08 for this prototype. It depicts a fictional place, not a verified photograph or a real event. The app labels its content as illustrative. Dioxus emits an optimized 960×640 JPEG from the source.

Exact generation prompt is embedded in the PNG under `impeccable:prompt`. Read it with:

The complete three-prompt set is also readable in [prompts.md](prompts.md).

```powershell
node <agent-skills>/impeccable/scripts/embed-prompt.mjs assets\images\neighborhood.png --read
```

The same fictional neighborhood is reused as a photo, video cover, and walking-event cover to demonstrate different activities. No playable video or real event registration is implied.

`shuna.png` (schnauzer, lavender background) and `patricia.png` (ginger cat, blush background) are fictional character portraits generated with the same built-in tool on 2026-09-08. Their exact individual prompts are embedded in each source PNG. Dioxus emits 128×128 JPEG avatars. They carry identity across the two demo windows; they are not product logos or actual users' profile photos.

## Additional fictional avatars — 2026-09-17

`golden.png` (golden retriever), `film.png` (river otter), and `soup.png` (brown bear) were commissioned for this project's remaining demonstration profiles and generated through GPT image generation. The creation record identifies the model as `gpt-image-2-medium`. They are fictional gouache-style character portraits, not photographs of real users or undocumented external downloads.

The current 512×512 PNGs are byte-identical to their introduction in commit `7a221383e6dc0de8237b2028c2249f9b8b39610e`:

| File | SHA-256 |
| --- | --- |
| `golden.png` | `cdf29b00569358537817f651d9d6672f580d7f2498f58be04f38061b2dce829d` |
| `film.png` | `676029cf7ea396c5b1273c92aedce2a1e0a4a28962c2e8808ea759b510e9f009` |
| `soup.png` | `1f625866578adeae3632c86caadcbabd5aa983ec86900567844fd5fea76d6692` |

Unlike the earlier three images, these PNGs have no embedded prompt, EXIF, XMP or C2PA chunks. The archived creation records retain only truncated prompts and processing arguments; original generation caches are unavailable. No complete prompt, exact processing recipe, raw-cache comparison or signed generation receipt is claimed. This provenance record does not guarantee copyrightability, uniqueness or generator terms. The project's dual license applies only to rights the project can grant and does not override third-party rights.
