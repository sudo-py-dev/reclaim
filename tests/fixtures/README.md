These deterministic ZIP fixtures were generated with Python's standard-library
`zipfile` writer, using a fixed 2020-01-01 timestamp. Entries use DEFLATE, except
`mimetype`, which is stored uncompressed as required for MIME recognition.

They contain small synthetic payloads, not complete Office documents, ebooks,
Java programs, or Android apps. They exercise real ZIP container structure and
package naming/MIME conventions without including third-party document content.
The DOCX fixture also has a comment longer than two scanner read chunks, with a
misleading EOCD signature near its end. All fixtures are project test data under
the repository's MIT license.

The `real.*`, `real89.gif`, `real-progressive.jpg`, `real-dib.ico`,
`real256.ico`, and animated image fixtures are small images generated with
Pillow from a project-owned RGB gradient. They contain valid encoded pixels,
not placeholder image bodies. ICO fixtures cover multiple PNG/DIB frames and
the zero-byte directory dimension that means 256 pixels. Test code mutates
copies in memory to exercise corruption and false-positive rejection.
