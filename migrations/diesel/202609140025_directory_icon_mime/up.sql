-- Keep the persisted directory-icon allowlist aligned with the bounded
-- signature/XML recognizers. Remote Content-Type headers are never trusted.
ALTER TABLE directory_icons DROP CONSTRAINT directory_icons_mime_check;
ALTER TABLE directory_icons ADD CONSTRAINT directory_icons_mime_check
    CHECK (mime IN (
        'image/png',
        'image/jpeg',
        'image/gif',
        'image/webp',
        'image/x-icon',
        'image/avif',
        'image/bmp',
        'image/tiff',
        'image/svg+xml'
    ));
