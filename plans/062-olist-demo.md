# Plan 062 — The Olist demo: a real star schema on the docs site

## Status

- **Priority**: P2 (adoption story, not correctness)
- **Effort**: M/L (one focused slice for the loader and model, one for the site page)
- **Risk**: MEDIUM (the dataset's own quirks are the work; the licensing flow must be kept clean)
- **Depends on**: 057 (qualifier and serving contract), 060 (site and certification)
- **Related**: 056 (behaviour reference), 053 (intake), 045 (intake fidelity)
- **Category**: product / demo

## Why

The bundled demo is synthetic and flat: dimensions live on the fact table, so it
exercises almost none of what the proxy exists for — relationships, grain,
fan-out, date roles, multi-fact models. The
[Olist Brazilian E-Commerce](https://www.kaggle.com/datasets/olistbr/brazilian-ecommerce)
dataset is the opposite: 100k orders across nine relational tables
(2016–2018), real keys, real gaps, and a shape analysts recognise.

What it would demonstrate that the synthetic demo cannot:

- **A real star**: `order_items` as the fact with customer, product, seller and
  date dimensions; `payments` as a **second fact** (the multi-fact case).
- **Three date roles on one fact** (purchased / approved / delivered), which
  makes YTD, QTD and prior-year measures land on real years.
- **The grain question**: revenue per order item versus per order; a
  `COUNT(DISTINCT order_id)` measure beside the additive ones.
- **The fan-out teaching moment**: a naive `orders ⋈ payments` doubles
  `payment_value`; the qualifier blocks it with the measured multiplier, and the
  shipped model shows the correct alternative. "No plausible wrong numbers",
  demonstrated on data people know.
- **Honest `qualify` findings**: orphan products and sellers, null delivery
  dates — real findings rather than ceremonial output.

## Licensing decision (decides the shape, not the value)

The dataset is **CC BY-NC-SA 4.0** (non-commercial, share-alike) and Kaggle
requires an account to download. Bundling it in an MIT repository, a release
artifact or a container is a licence conflict, and scripted download would break
the no-accounts, air-gapped, no-runtime-internet posture.

**Therefore: ship the model and the loader, never the bytes.**

- `projects/olist/` contains the upstream SQL (CSV → typed tables → dimensions
  and facts), the projection config and the expected findings. No data files.
- Setup is a documented, one-time step: the user downloads from Kaggle under
  their own acceptance of the terms, then points the loader at the CSVs. The
  proxy itself never fetches anything.
- CI stays on the bundled synthetic project. Optionally a tiny synthetic fixture
  that *mimics* Olist's shape (two facts, three date roles, a category lookup)
  so relationship and date-role tests do not need a download.

If a bundled or CI-shippable demo is ever wanted, generate TPC-DS data instead:
same star shape, no licence questions, reproducible. Olist stays the
recognisable human story; generated data stays the pipeline's.

## A. The upstream (loader)

`projects/olist/upstream/*.sql`, runnable with one command against the CSVs:

- `read_csv_auto` for the nine tables, with explicit types where Olist's text
  timestamps and mixed nulls need it;
- `dim_date` — one row per day over the observed range, plus the role columns
  the time-intelligence config expects;
- `dim_customer` (state → city hierarchy), `dim_product` (joined to the English
  category translation), `dim_seller` (state);
- `order_items` (fact: price, freight, quantity-ish counts) and `payments`
  (fact: value, type, installments);
- the pipeline writes `olist.duckdb`, which the proxy then serves read-only.

## B. The projection

`projects/olist/proxy-config.json` (later: the plan 057 contract), chosen to
exercise the surface:

- fact grain `order_items`; measures `Revenue = SUM(price)`,
  `Freight = SUM(freight_value)`, `Items = SUM(...)`,
  `Orders = COUNT(DISTINCT order_id)` (a distinct-count measure — verify the
  aggregator path first) and `Avg Review = AVG(review_score)` (non-additive, so
  the qualifier demands an oracle rather than assuming);
- the three purchase/approved/delivered date roles, each mapped to its column;
- `payments` as a second fact (payment value by type), demonstrating measure
  groups;
- relationships with declared cardinality, so the qualifier's uniqueness and
  fan-out checks have real work to do.

## C. The pitfalls, documented

A short section (site page or plan 056 reference) with the two findings the demo
produces: the fan-out multiplier on the naive join, and the orphan counts. This
is the honest "here is what the qualifier catches" story, and it doubles as the
qualifier's best documentation.

## D. The site page

A demo page: fetch the CSVs, run the loader, start the proxy, connect Excel,
then a scripted walkthrough (category by state with a date filter, a YTD
measure, a `CUBEVALUE` cell), and finally `qualify`'s output with the expected
findings. Link it from the README's "See it work" as the "or try it on real
public data" path.

## E. Gates

- `scripts/probe-*` and the corpus replay stay on the synthetic project; nothing
  in CI may depend on a download.
- If the optional shape-mimicking fixture lands, it runs in the normal test
  suite and covers the two facts, three date roles and the category lookup.

## Scope

**In**: loader SQL, the projection, the documented pitfalls, the site demo page,
a README link, optionally the synthetic shape fixture.

**Out**: vendoring the data, Kaggle automation, any runtime download, changes to
the proxy or the qualifier beyond what the demo reveals.

## Done criteria

- On a clean machine, a user who has downloaded the CSVs runs one loader command,
  starts the proxy, and pivots Olist in Excel — the walkthrough on the site page
  is reproducible from scratch.
- `qualify` on the shipped projection reports **only** the expected findings
  (orphans, nothing that blocks), and the naive-join variant is documented with
  its blocked verdict and multiplier.
- No Olist bytes in the repository, release artifacts or container images; CI
  unchanged.
- The site page is published and the build stays green (`check-links`,
  `check-claims`).

## STOP conditions

- If Kaggle's terms as read at implementation time do not permit the
  fetch-it-yourself flow as described, stop and switch to generated TPC-DS data
  — do not ship the bytes.
- If a demo requirement forces a change to the proxy or the qualifier rather
  than the projection, stop and record it as a separate plan: the demo must
  consume the product, not shape it.
