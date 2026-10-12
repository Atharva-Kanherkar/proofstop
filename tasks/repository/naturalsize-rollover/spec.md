Repair humanize/filesize.py in the provided historical humanize runtime.

naturalsize(value, binary=False, gnu=False, format="%.1f") formats byte counts. When the formatted mantissa rounds to the unit base (1000 decimal, 1024 binary/GNU), advance to the next suffix if one exists and format the scaled value there. Preserve signs, string-valued numeric input, custom precision, ordinary values, single-byte and small-byte formatting, GNU spacing, and the largest available suffix. Do not change other modules. Use the actual humanize.naturalsize API, not a replacement wrapper.

Each visible input is [value, binary, gnu, format], passed as positional arguments to the public API in the default English locale. The replacement file includes literal triple-backtick examples inside a docstring, so return the COMPLETE file in exactly one Python code block delimited by FOUR backticks (````python on the opening line, ```` on the closing line). Do not omit unrelated declarations or imports.
