# Conversion Inventory

## Summary

- **Tables**: 19
- **Fact tables**: `dw_fys F_Undersökning`
- **Dimension tables**: `dw_fys D_Beställare`, `dw_fys D_Fakturamottagare`, `dw_fys D_Patient`, `dw_fys D_Produkt`, `dw_fys D_Remissgrupp`, `dw_fys D_Remisskoder`, `dw_fys D_Remisstatus`, `dw_fys D_Signerare`, `dw_fys D_Utförare`
- **Date-role tables**: `dw_fys Kalender_Beställningsdatum`, `dw_fys Kalender_Bokningsdatum`, `dw_fys Kalender_Måldatum`, `dw_fys Kalender_Remissdatum`, `dw_fys Kalender_Signeringsdatum`, `dw_fys Kalender_Undersökningsslut`, `dw_fys Kalender_Undersökningsstart`
- **Calculated tables**: `Daggrupp`
- **M-partition tables**: `CARMAe`, `dw_fys D_Beställare`, `dw_fys D_Fakturamottagare`, `dw_fys D_Patient`, `dw_fys D_Produkt`, `dw_fys D_Remissgrupp`, `dw_fys D_Remisskoder`, `dw_fys D_Remisstatus`, `dw_fys D_Signerare`, `dw_fys D_Utförare`, `dw_fys F_Undersökning`, `dw_fys Kalender_Beställningsdatum`, `dw_fys Kalender_Bokningsdatum`, `dw_fys Kalender_Måldatum`, `dw_fys Kalender_Remissdatum`, `dw_fys Kalender_Signeringsdatum`, `dw_fys Kalender_Undersökningsslut`, `dw_fys Kalender_Undersökningsstart`
- **Relationships**: 16
- **Measures**: 33 (simple: 22, sql_fallback: 11, manual: 0)

## Relationships

| From | From Col | To | To Col |
|---|---|---|---|
| dw_fys F_Undersökning | BeställareId | dw_fys D_Beställare | BeställareId |
| dw_fys F_Undersökning | FakturamottagareId | dw_fys D_Fakturamottagare | FakturamottagareId |
| dw_fys F_Undersökning | PatientId | dw_fys D_Patient | PatientId |
| dw_fys F_Undersökning | ProduktId | dw_fys D_Produkt | ProduktId |
| dw_fys F_Undersökning | RemissgruppId | dw_fys D_Remissgrupp | RemissgruppId |
| dw_fys F_Undersökning | RemisskoderId | dw_fys D_Remisskoder | RemisskoderId |
| dw_fys F_Undersökning | RemisstatusId | dw_fys D_Remisstatus | RemisstatusId |
| dw_fys F_Undersökning | SignerareId | dw_fys D_Signerare | SignerareId |
| dw_fys F_Undersökning | UtförareId | dw_fys D_Utförare | UtförareId |
| dw_fys F_Undersökning | Beställningsdatum | dw_fys Kalender_Beställningsdatum | Beställningsdatum |
| dw_fys F_Undersökning | Bokningsdatum | dw_fys Kalender_Bokningsdatum | Bokningsdatum |
| dw_fys F_Undersökning | Måldatum_från | dw_fys Kalender_Måldatum | Måldatum |
| dw_fys F_Undersökning | Remissdatum | dw_fys Kalender_Remissdatum | Remissdatum |
| dw_fys F_Undersökning | Signeringsdatum | dw_fys Kalender_Signeringsdatum | Signeringsdatum |
| dw_fys F_Undersökning | Undersökningsslut | dw_fys Kalender_Undersökningsslut | Undersökningsslut |
| dw_fys F_Undersökning | Undersökningsstart | dw_fys Kalender_Undersökningsstart | Undersökningsstart |

## Tables

### CARMAe

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Beskrivning | string | Beskrivning | false |
| Kategori | string | Kategori | false |
| Källa | string | Källa | true |
| Namn | string | Namn | false |
| Namn och beskrivning | string | Namn och beskrivning | false |
| ObjektKod | int64 | ObjektKod | true |
| Objektnyckel | string | Objektnyckel | false |
| ObjektschemaKod | string | ObjektschemaKod | true |
| Objekttyp | string | Objekttyp | false |
| Skapad | dateTime | Skapad | true |
| Status | string | Status | false |

### Daggrupp

**Partitions**: CalculatedTable 1 (calculated)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Från | int64 | [Från] | false |
| Id | int64 | [Id] | false |
| Namn | string | [Namn] | false |
| Till | int64 | [Till] | false |

### dw_fys D_Beställare

_Beställare av undersökningar_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Adress | string | Adress | false |
| Beställare | string | Beställare | false |
| BeställareId | int64 | BeställareId | true |
| BeställareKod | string | BeställareKod | false |
| Intern%2fExtern | string | Intern/Extern | false |
| Kostnadsställe | string | Kostnadsställe | false |
| Källa | string | Källa | true |
| Postadress | string | Postadress | false |
| Skapad | dateTime | Skapad | true |
| Tillfällig adress | string | Tillfällig adress | false |
| VårdtypKod | string | VårdtypKod | false |

### dw_fys D_Fakturamottagare

_Fakturamottagare av undersökning_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Adress | string | Adress | false |
| Fakturamottagare | string | Fakturamottagare | false |
| FakturamottagareId | int64 | FakturamottagareId | true |
| FakturamottagareKod | string | FakturamottagareKod | false |
| Intern%2fExtern | string | Intern/Extern | false |
| Kostnadsställe | string | Kostnadsställe | false |
| Källa | string | Källa | true |
| Postadress | string | Postadress | false |
| Skapad | dateTime | Skapad | true |
| Tillfällig adress | string | Tillfällig adress | false |
| VårdtypKod | string | VårdtypKod | false |

### dw_fys D_Patient

_Patient som undersöks, av GDPR-skäl döljs personuppgifter_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Efternamn | string | Efternamn | true |
| Förnamn | string | Förnamn | true |
| Källa | string | Källa | true |
| PatientId | int64 | PatientId | false |
| PatientKod | string | PatientKod | true |
| Skapad | dateTime | Skapad | true |

### dw_fys D_Produkt

_Debitterbar produkt samt information som kopplas till detta_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| BIS Kod | string | BIS Kod | false |
| Källa | string | Källa | true |
| Metodgrupp | string | Metodgrupp | false |
| ProduktId | int64 | ProduktId | true |
| ProduktKod | string | ProduktKod | false |
| Produktbeskrivning | string | Produktbeskrivning | false |
| Produktgrupp | string | Produktgrupp | false |
| SLL Kod | string | SLL Kod | false |
| Sektion | string | Sektion | false |
| Skapad | dateTime | Skapad | true |
| Vårdval undergrupp | string | Vårdval undergrupp | false |
| Vårdvalsavtal | string | Vårdvalsavtal | false |

### dw_fys D_Remissgrupp

_Remissgruppering_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Källa | string | Källa | true |
| Remissgrupp | string | Remissgrupp | false |
| RemissgruppId | int64 | RemissgruppId | true |
| RemissgruppKod | int64 | RemissgruppKod | false |
| Skapad | dateTime | Skapad | true |

### dw_fys D_Remisskoder

_Olika koder/attribut kopplade till remiss, t.ex. akut, modifieringsattribut, undersökningsstatus, utebliven_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Akut | string | Akut | false |
| Källa | string | Källa | true |
| Modifieringsattribut | string | Modifieringsattribut | false |
| Remissattribut | string | Remissattribut | false |
| RemisskoderId | int64 | RemisskoderId | true |
| RemisskoderKod | string | RemisskoderKod | false |
| Skapad | dateTime | Skapad | true |
| Undersökningsstatus | string | Undersökningsstatus | false |
| Undersökningsvikt | string | Undersökningsvikt | false |
| Utebliven | string | Utebliven | false |

### dw_fys D_Remisstatus

_Status på remissen_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Källa | string | Källa | true |
| Remisstatus | string | Remisstatus | false |
| RemisstatusId | int64 | RemisstatusId | true |
| RemisstatusKod | int64 | RemisstatusKod | false |
| Skapad | dateTime | Skapad | true |

### dw_fys D_Signerare

_Personal som signerar undersökning_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Befattning, signerare | string | Befattning, signerare | false |
| Efternamn, signerare | string | Efternamn, signerare | false |
| Förnamn, signerare | string | Förnamn, signerare | false |
| Källa | string | Källa | true |
| Signerare | string | Signerare | false |
| SignerareId | int64 | SignerareId | true |
| SignerareKod | string | SignerareKod | false |
| Skapad | dateTime | Skapad | true |

### dw_fys D_Utförare

_Personal som utför undersökning_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Befattning, utförare | string | Befattning, utförare | false |
| Efternamn, utförare | string | Efternamn, utförare | false |
| Förnamn, utförare | string | Förnamn, utförare | false |
| Källa | string | Källa | true |
| Skapad | dateTime | Skapad | true |
| Utförare | string | Utförare | false |
| UtförareId | int64 | UtförareId | true |
| UtförareKod | string | UtförareKod | false |

### dw_fys F_Undersökning

_Faktatabell för undersökningar, varje rad motsvarar antigen en inkommen remiss, eltt bokat besök eller en debitterad produkt. Endast fakturerbara produkter inkluderas._

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| BeställareId | int64 | BeställareId | true |
| Beställning till signering | double | Beställning till signering | false |
| Beställning till undersökningsstart | double | Beställning till undersökningsstart | false |
| Beställning till undersökningsstart (Akut & timmar) | double | Beställning till undersökningsstart (Akut & timmar) | true |
| Beställning till undersökningsstart (Akut) | double | Beställning till undersökningsstart (Akut) | true |
| Beställning till undersökningsstart (timmar) | double | Beställning till undersökningsstart (timmar) | false |
| Beställningsdatum | dateTime | Beställningsdatum | true |
| Beställningsdatumtid | dateTime | Beställningsdatumtid | false |
| Beställningstimme | int64 | Beställningstimme | false |
| Bokningsdatum | dateTime | Bokningsdatum | true |
| Bokningsdatumtid | dateTime | Bokningsdatumtid | false |
| FakturamottagareId | int64 | FakturamottagareId | true |
| Flagga Akuten | int64 | Flagga Akuten | true |
| Källa | string | Källa | true |
| Måldatum_från | dateTime | Måldatum_från | true |
| Måldatum_till | dateTime | Måldatum_till | true |
| Måldatumtid_från | dateTime | Måldatumtid_från | true |
| Måldatumtid_till | dateTime | Måldatumtid_till | true |
| PatientId | int64 | PatientId | true |
| ProduktId | int64 | ProduktId | true |
| Registrering till signering | double | Registrering till signering | false |
| Registreringsdatum | dateTime | Registreringsdatum | true |
| Registreringsdatumtid | dateTime | Registreringsdatumtid | false |
| Remiss till bokning | double | Remiss till bokning | false |
| Remiss till idag | int64 | Remiss till idag | false |
| Remiss till undersökningsstart | double | Remiss till undersökningsstart | false |
| Remissdatum | dateTime | Remissdatum | true |
| Remissdatumtid | dateTime | Remissdatumtid | false |
| RemissgruppId | int64 | RemissgruppId | true |
| RemisskoderId | int64 | RemisskoderId | true |
| Remissnummer | string | RemissKod | false |
| RemisstatusId | int64 | RemisstatusId | true |
| Remisstimme | int64 | Remisstimme | false |
| SignerareId | int64 | SignerareId | true |
| Signeringsdatum | dateTime | Signeringsdatum | true |
| Signeringsdatumtid | dateTime | Signeringsdatumtid | false |
| Signeringsstatus | string | Signeringsstatus | false |
| Signeringstimme | int64 | Signeringstimme | false |
| Skapad | dateTime | Skapad | false |
| Sortering väntetidsgrupp | int64 | Sortering väntetidsgrupp | true |
| Tabell | string | Tabell | true |
| UndersökningId | int64 | UndersökningId | true |
| UndersökningKod | string | UndersökningKod | false |
| Undersökningsslut | dateTime | Undersökningsslut | true |
| Undersökningsslut till signering | double | Undersökningsslut till signering | false |
| Undersökningsslut till signering - ej akut | double | Undersökningsslut till signering - ej akut | true |
| Undersökningsslutdatumtid | dateTime | Undersökningsslutdatumtid | false |
| Undersökningssluttimme | int64 | Undersökningssluttimme | false |
| Undersökningsstart | dateTime | Undersökningsstart | true |
| Undersökningsstartdatumtid | dateTime | Undersökningsstartdatumtid | false |
| Undersökningsstarttimme | int64 | Undersökningsstarttimme | false |
| UtförareId | int64 | UtförareId | true |
| Väntetidsgruppering | string | Väntetidsgruppering | false |

| Measure | Classification |
|---|---|
| Andel signerade DVT Under två timmar | simple |
| Andel under fyra timmar | simple |
| Antal bokade remisser | simple |
| Antal obokade remisser | simple |
| Antal osignerade utförda undersökningar | simple |
| Antal patienter | simple |
| Antal remisser | simple |
| Antal remisser akuta inneliggande | simple |
| Antal remisser akuta inneliggande (Under fyra timmar) | simple |
| Antal remisser i urval | simple |
| Antal remisser som väntat %3e30 dagar | simple |
| Antal remisser som väntat %3e60 dagar | simple |
| Antal remisser som väntat %3e90 dagar | simple |
| Antal remissiser med tid inom 4 veckor | simple |
| Antal saknade remisser i underlag | simple |
| Antal signerade DVT-remisser | sql_fallback |
| Antal undersökningar | simple |
| Antal uteblivna remisser | simple |
| Antal utförda remisser | simple |
| Antal utförda remisser (ack Kvartal) CY | sql_fallback |
| Antal utförda remisser (ack Kvartal) CY-1 | sql_fallback |
| Antal utförda remisser (ack Månad) CY-1 | sql_fallback |
| Antal utförda remisser (ack månad) CY | sql_fallback |
| Antal utförda remisser (ack vecka) CY | sql_fallback |
| Antal utförda remisser (ack vecka) CY-1 | sql_fallback |
| Medeltid Undersökningsslut till signering (ej akut) | sql_fallback |
| Medeltid registrering till signering | simple |
| Medeltid undersökningsslut till signering | simple |
| Median beställning till undersökning | sql_fallback |
| Median beställning till undersökning (Akut & timmar) | sql_fallback |
| Median beställning till undersökning (Akut) | sql_fallback |
| Referens DVT remiss | simple |
| Tid inom 4 veckor från skickad remiss | simple |

### dw_fys Kalender_Beställningsdatum

_Kalender kopplad till beställningsdatum, hämtas fryn Fysweb om data saknas i Fyspaf_

**Partitions**: Partition (m)

| Column | Type | Source | Hidden |
|---|---|---|---|
| Beställningsdatum | dateTime | Beställningsdatum | false |
| CapioVecka | int64 | CapioVecka | false |
| DagMånad | int64 | DagMånad | false |
| DagNamn | string | DagNamn | false |
| DagNamn kort | string | DagNamn kort | false |
| DagTyp | string | DagTyp | false |
| DagTypId | int64 | DagTypId | false |
| DagÅr | int64 | DagÅr | false |
| Datum | dateTime | Datum | false |
| Kvartal | int64 | Kvartal | false |
| Kvartal räkenskapsår | string | Kvartal räkenskapsår | false |
| KvartalNamn | string | KvartalNamn | false |
| Månad | int64 | Månad | false |
| Månad räkenskapsår | string | Månad räkenskapsår | false |
| MånadNamn | string | MånadNamn | false |
| MånadNamn kort | string | MånadNamn kort | false |
| Vecka | int64 | Vecka | false |
| VeckodagsSiffra | int64 | VeckodagsSiffra | false |
| År | int64 | År | false |
| År räkenskapsår | string | År räkenskapsår | false |
| ÅrMånad | string | ÅrMånad | false |
| ÅrMånad räkenskapsår | string | ÅrMånad räkenskapsår | false |

### dw_fys Kalender_Bokningsdatum

_Kalender för bokningsdatum_

**Partitions**: Partition (m)

**Hierarchies**: Datumhierarki

| Column | Type | Source | Hidden |
|---|---|---|---|
| Bokningsdatum | dateTime | Bokningsdatum | false |
| CapioVecka | int64 | CapioVecka | false |
| DagMånad | int64 | DagMånad | false |
| DagNamn | string | DagNamn | false |
| DagNamn kort | string | DagNamn kort | false |
| DagTyp | string | DagTyp | false |
| DagTypId | int64 | DagTypId | false |
| DagÅr | int64 | DagÅr | false |
| Datum | dateTime | Datum | false |
| Kvartal | int64 | Kvartal | false |
| Kvartal räkenskapsår | string | Kvartal räkenskapsår | false |
| KvartalNamn | string | KvartalNamn | false |
| Månad | int64 | Månad | false |
| Månad räkenskapsår | string | Månad räkenskapsår | false |
| MånadNamn | string | MånadNamn | false |
| MånadNamn kort | string | MånadNamn kort | false |
| Vecka | int64 | Vecka | false |
| VeckodagsSiffra | int64 | VeckodagsSiffra | false |
| År | int64 | År | false |
| År räkenskapsår | string | År räkenskapsår | false |
| ÅrMånad | string | ÅrMånad | false |
| ÅrMånad räkenskapsår | string | ÅrMånad räkenskapsår | false |

### dw_fys Kalender_Måldatum

**Partitions**: Partition (m)

**Hierarchies**: Datumhierarki

| Column | Type | Source | Hidden |
|---|---|---|---|
| CapioVecka | int64 | CapioVecka | false |
| DagMånad | int64 | DagMånad | false |
| DagNamn | string | DagNamn | false |
| DagNamn kort | string | DagNamn kort | false |
| DagTyp | string | DagTyp | false |
| DagTypId | int64 | DagTypId | false |
| DagÅr | int64 | DagÅr | false |
| DagÅr räkenskapsår | int64 | DagÅr räkenskapsår | false |
| Datum | dateTime | Datum | true |
| Kvartal | int64 | Kvartal | false |
| Kvartal räkenskapsår | string | Kvartal räkenskapsår | false |
| KvartalNamn | string | KvartalNamn | false |
| Måldatum | dateTime | Måldatum | false |
| Månad | int64 | Månad | false |
| Månad räkenskapsår | string | Månad räkenskapsår | false |
| MånadNamn | string | MånadNamn | false |
| MånadNamn kort | string | MånadNamn kort | false |
| Vecka | int64 | Vecka | false |
| VeckodagsSiffra | int64 | VeckodagsSiffra | false |
| År | int64 | År | false |
| År räkenskapsår | string | År räkenskapsår | false |
| ÅrMånad | string | ÅrMånad | false |
| ÅrMånad räkenskapsår | string | ÅrMånad räkenskapsår | false |

### dw_fys Kalender_Remissdatum

_Kalender för när remiss skickades_

**Partitions**: Partition (m)

**Hierarchies**: Datumhierarki

| Column | Type | Source | Hidden |
|---|---|---|---|
| CapioVecka | int64 | CapioVecka | false |
| DagMånad | int64 | DagMånad | false |
| DagNamn | string | DagNamn | false |
| DagNamn kort | string | DagNamn kort | false |
| DagTyp | string | DagTyp | false |
| DagTypId | int64 | DagTypId | false |
| DagÅr | int64 | DagÅr | false |
| Datum | dateTime | Datum | false |
| Kvartal | int64 | Kvartal | false |
| Kvartal räkenskapsår | string | Kvartal räkenskapsår | false |
| KvartalNamn | string | KvartalNamn | false |
| Månad | int64 | Månad | false |
| Månad räkenskapsår | string | Månad räkenskapsår | false |
| MånadNamn | string | MånadNamn | false |
| MånadNamn kort | string | MånadNamn kort | false |
| Remissdatum | dateTime | Remissdatum | false |
| Vecka | int64 | Vecka | false |
| VeckodagsSiffra | int64 | VeckodagsSiffra | false |
| År | int64 | År | false |
| År räkenskapsår | string | År räkenskapsår | false |
| ÅrMånad | string | ÅrMånad | false |
| ÅrMånad räkenskapsår | string | ÅrMånad räkenskapsår | false |

### dw_fys Kalender_Signeringsdatum

_Kalender för när undersökning signerades_

**Partitions**: Partition (m)

**Hierarchies**: Datumhierarki

| Column | Type | Source | Hidden |
|---|---|---|---|
| CapioVecka | int64 | CapioVecka | false |
| DagMånad | int64 | DagMånad | false |
| DagNamn | string | DagNamn | false |
| DagNamn kort | string | DagNamn kort | false |
| DagTyp | string | DagTyp | false |
| DagTypId | int64 | DagTypId | false |
| DagÅr | int64 | DagÅr | false |
| Datum | dateTime | Datum | false |
| Kvartal | int64 | Kvartal | false |
| Kvartal räkenskapsår | string | Kvartal räkenskapsår | false |
| KvartalNamn | string | KvartalNamn | false |
| Månad | int64 | Månad | false |
| Månad räkenskapsår | string | Månad räkenskapsår | false |
| MånadNamn | string | MånadNamn | false |
| MånadNamn kort | string | MånadNamn kort | false |
| Signeringsdatum | dateTime | Signeringsdatum | false |
| Vecka | int64 | Vecka | false |
| VeckodagsSiffra | int64 | VeckodagsSiffra | false |
| År | int64 | År | false |
| År räkenskapsår | string | År räkenskapsår | false |
| ÅrMånad | string | ÅrMånad | false |
| ÅrMånad räkenskapsår | string | ÅrMånad räkenskapsår | false |

### dw_fys Kalender_Undersökningsslut

_Kalender för när undersökning avslutades_

**Partitions**: Partition (m)

**Hierarchies**: Datumhierarki

| Column | Type | Source | Hidden |
|---|---|---|---|
| CapioVecka | int64 | CapioVecka | false |
| DagMånad | int64 | DagMånad | false |
| DagNamn | string | DagNamn | false |
| DagNamn kort | string | DagNamn kort | false |
| DagTyp | string | DagTyp | false |
| DagTypId | int64 | DagTypId | false |
| DagÅr | int64 | DagÅr | false |
| Datum | dateTime | Datum | false |
| Kvartal | int64 | Kvartal | false |
| Kvartal räkenskapsår | string | Kvartal räkenskapsår | false |
| KvartalNamn | string | KvartalNamn | false |
| Månad | int64 | Månad | false |
| Månad räkenskapsår | string | Månad räkenskapsår | false |
| MånadNamn | string | MånadNamn | false |
| MånadNamn kort | string | MånadNamn kort | false |
| Undersökningsslut | dateTime | Undersökningsslut | false |
| Vecka | int64 | Vecka | false |
| Veckodag dynamisk | string | Veckodag dynamisk | false |
| VeckodagsSiffra | int64 | VeckodagsSiffra | false |
| Veckodagsiffra, dynamisk | double | Veckodagsiffra, dynamisk | false |
| År | int64 | År | false |
| År räkenskapsår | string | År räkenskapsår | false |
| ÅrMånad | string | ÅrMånad | false |
| ÅrMånad räkenskapsår | string | ÅrMånad räkenskapsår | false |

### dw_fys Kalender_Undersökningsstart

_Kalender för när undersökning startades_

**Partitions**: Partition (m)

**Hierarchies**: Datumhierarki

| Column | Type | Source | Hidden |
|---|---|---|---|
| CapioVecka | int64 | CapioVecka | false |
| DagMånad | int64 | DagMånad | false |
| DagNamn | string | DagNamn | false |
| DagNamn kort | string | DagNamn kort | false |
| DagTyp | string | DagTyp | false |
| DagTypId | int64 | DagTypId | false |
| DagÅr | int64 | DagÅr | false |
| Datum | dateTime | Datum | false |
| Kvartal | int64 | Kvartal | false |
| Kvartal räkenskapsår | string | Kvartal räkenskapsår | false |
| KvartalNamn | string | KvartalNamn | false |
| Månad | int64 | Månad | false |
| Månad räkenskapsår | string | Månad räkenskapsår | false |
| MånadNamn | string | MånadNamn | false |
| MånadNamn kort | string | MånadNamn kort | false |
| Undersökningsstart | dateTime | Undersökningsstart | false |
| Vecka | int64 | Vecka | false |
| VeckodagsSiffra | int64 | VeckodagsSiffra | false |
| År | int64 | År | false |
| År räkenskapsår | string | År räkenskapsår | false |
| ÅrMånad | string | ÅrMånad | false |
| ÅrMånad räkenskapsår | string | ÅrMånad räkenskapsår | false |

## Roles

- **fys_läsbehörighet**: Läsbehörighet för fys-data

