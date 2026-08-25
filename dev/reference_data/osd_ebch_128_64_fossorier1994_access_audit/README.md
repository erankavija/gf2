# Fossorier 1995 article access audit

This directory records the evidence behind the access-status statement for
`@/citation/Fossorier1995`, DOI `10.1109/18.412683`. It does not supply or
claim access to the article.

## Unpaywall query

The raw response in
[`unpaywall_10.1109_18.412683.json`](unpaywall_10.1109_18.412683.json)
was retrieved on 2026-08-25 with:

```console
curl -fL -o unpaywall_10.1109_18.412683.json \
  'https://api.unpaywall.org/v2/10.1109/18.412683?email=vesa.kaskivuo@iki.fi'
```

The 1,051-byte response has SHA-256
`0d434843622a46cadd19b8e82b3098756d719058bebbea064f510ce123ada7d1`.
It reports `is_oa: false`, `oa_status: "closed"`,
`has_repository_copy: false`, no best or first OA location, and empty
`oa_locations` and `oa_locations_embargoed` arrays. These are the API's
reported fields, not an inference that no copy can exist anywhere.

## ResearchGate lead

[`researchgate_lead_identification.md`](researchgate_lead_identification.md)
records the title-page identification of the file delivered by the advertised
ResearchGate “full text” lead. The file is a different 1999 work with five
authors. Its hash and metadata are recorded, but the PDF itself is not copied
into the repository.

The audit establishes that this lead is a title-collision false positive; it
does not establish an exhaustive search of every possible repository. The
1995 IEEE Transactions on Information Theory article remains unobtained by
this audit.
