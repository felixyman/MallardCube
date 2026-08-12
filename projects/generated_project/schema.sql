-- Generated from Tabular Editor model
-- Data loading via M partitions must be done manually.

CREATE TABLE IF NOT EXISTS dw_fys_f_undersökning (
    beställareid BIGINT,
    beställning_till_signering DOUBLE,
    beställning_till_undersökningsstart DOUBLE,
    beställning_till_undersökningsstart_(akut_&_timmar) DOUBLE,
    beställning_till_undersökningsstart_(akut) DOUBLE,
    beställning_till_undersökningsstart_(timmar) DOUBLE,
    beställningsdatum TIMESTAMP,
    beställningsdatumtid TIMESTAMP,
    beställningstimme BIGINT,
    bokningsdatum TIMESTAMP,
    bokningsdatumtid TIMESTAMP,
    fakturamottagareid BIGINT,
    flagga_akuten BIGINT,
    källa VARCHAR,
    måldatum_från TIMESTAMP,
    måldatum_till TIMESTAMP,
    måldatumtid_från TIMESTAMP,
    måldatumtid_till TIMESTAMP,
    patientid BIGINT,
    produktid BIGINT,
    registrering_till_signering DOUBLE,
    registreringsdatum TIMESTAMP,
    registreringsdatumtid TIMESTAMP,
    remiss_till_bokning DOUBLE,
    remiss_till_idag BIGINT,
    remiss_till_undersökningsstart DOUBLE,
    remissdatum TIMESTAMP,
    remissdatumtid TIMESTAMP,
    remissgruppid BIGINT,
    remisskoderid BIGINT,
    remisskod VARCHAR,
    remisstatusid BIGINT,
    remisstimme BIGINT,
    signerareid BIGINT,
    signeringsdatum TIMESTAMP,
    signeringsdatumtid TIMESTAMP,
    signeringsstatus VARCHAR,
    signeringstimme BIGINT,
    skapad TIMESTAMP,
    sortering_väntetidsgrupp BIGINT,
    tabell VARCHAR,
    undersökningid BIGINT,
    undersökningkod VARCHAR,
    undersökningsslut TIMESTAMP,
    undersökningsslut_till_signering DOUBLE,
    undersökningsslut_till_signering___ej_akut DOUBLE,
    undersökningsslutdatumtid TIMESTAMP,
    undersökningssluttimme BIGINT,
    undersökningsstart TIMESTAMP,
    undersökningsstartdatumtid TIMESTAMP,
    undersökningsstarttimme BIGINT,
    utförareid BIGINT,
    väntetidsgruppering VARCHAR
);
-- FACT TABLE: dw_fys F_Undersökning

CREATE TABLE IF NOT EXISTS dw_fys_d_beställare (
    adress VARCHAR,
    beställare VARCHAR,
    beställareid BIGINT,
    beställarekod VARCHAR,
    intern/extern VARCHAR,
    kostnadsställe VARCHAR,
    källa VARCHAR,
    postadress VARCHAR,
    skapad TIMESTAMP,
    tillfällig_adress VARCHAR,
    vårdtypkod VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_fakturamottagare (
    adress VARCHAR,
    fakturamottagare VARCHAR,
    fakturamottagareid BIGINT,
    fakturamottagarekod VARCHAR,
    intern/extern VARCHAR,
    kostnadsställe VARCHAR,
    källa VARCHAR,
    postadress VARCHAR,
    skapad TIMESTAMP,
    tillfällig_adress VARCHAR,
    vårdtypkod VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_patient (
    efternamn VARCHAR,
    förnamn VARCHAR,
    källa VARCHAR,
    patientid BIGINT,
    patientkod VARCHAR,
    skapad TIMESTAMP
);

CREATE TABLE IF NOT EXISTS dw_fys_d_produkt (
    bis_kod VARCHAR,
    källa VARCHAR,
    metodgrupp VARCHAR,
    produktid BIGINT,
    produktkod VARCHAR,
    produktbeskrivning VARCHAR,
    produktgrupp VARCHAR,
    sll_kod VARCHAR,
    sektion VARCHAR,
    skapad TIMESTAMP,
    vårdval_undergrupp VARCHAR,
    vårdvalsavtal VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_remissgrupp (
    källa VARCHAR,
    remissgrupp VARCHAR,
    remissgruppid BIGINT,
    remissgruppkod BIGINT,
    skapad TIMESTAMP
);

CREATE TABLE IF NOT EXISTS dw_fys_d_remisskoder (
    akut VARCHAR,
    källa VARCHAR,
    modifieringsattribut VARCHAR,
    remissattribut VARCHAR,
    remisskoderid BIGINT,
    remisskoderkod VARCHAR,
    skapad TIMESTAMP,
    undersökningsstatus VARCHAR,
    undersökningsvikt VARCHAR,
    utebliven VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_remisstatus (
    källa VARCHAR,
    remisstatus VARCHAR,
    remisstatusid BIGINT,
    remisstatuskod BIGINT,
    skapad TIMESTAMP
);

CREATE TABLE IF NOT EXISTS dw_fys_d_signerare (
    befattning,_signerare VARCHAR,
    efternamn,_signerare VARCHAR,
    förnamn,_signerare VARCHAR,
    källa VARCHAR,
    signerare VARCHAR,
    signerareid BIGINT,
    signerarekod VARCHAR,
    skapad TIMESTAMP
);

CREATE TABLE IF NOT EXISTS dw_fys_d_utförare (
    befattning,_utförare VARCHAR,
    efternamn,_utförare VARCHAR,
    förnamn,_utförare VARCHAR,
    källa VARCHAR,
    skapad TIMESTAMP,
    utförare VARCHAR,
    utförareid BIGINT,
    utförarekod VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_beställningsdatum (
    beställningsdatum TIMESTAMP,
    capiovecka BIGINT,
    dagmånad BIGINT,
    dagnamn VARCHAR,
    dagnamn_kort VARCHAR,
    dagtyp VARCHAR,
    dagtypid BIGINT,
    dagår BIGINT,
    datum TIMESTAMP,
    kvartal BIGINT,
    kvartal_räkenskapsår VARCHAR,
    kvartalnamn VARCHAR,
    månad BIGINT,
    månad_räkenskapsår VARCHAR,
    månadnamn VARCHAR,
    månadnamn_kort VARCHAR,
    vecka BIGINT,
    veckodagssiffra BIGINT,
    år BIGINT,
    år_räkenskapsår VARCHAR,
    årmånad VARCHAR,
    årmånad_räkenskapsår VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_bokningsdatum (
    bokningsdatum TIMESTAMP,
    capiovecka BIGINT,
    dagmånad BIGINT,
    dagnamn VARCHAR,
    dagnamn_kort VARCHAR,
    dagtyp VARCHAR,
    dagtypid BIGINT,
    dagår BIGINT,
    datum TIMESTAMP,
    kvartal BIGINT,
    kvartal_räkenskapsår VARCHAR,
    kvartalnamn VARCHAR,
    månad BIGINT,
    månad_räkenskapsår VARCHAR,
    månadnamn VARCHAR,
    månadnamn_kort VARCHAR,
    vecka BIGINT,
    veckodagssiffra BIGINT,
    år BIGINT,
    år_räkenskapsår VARCHAR,
    årmånad VARCHAR,
    årmånad_räkenskapsår VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_måldatum (
    capiovecka BIGINT,
    dagmånad BIGINT,
    dagnamn VARCHAR,
    dagnamn_kort VARCHAR,
    dagtyp VARCHAR,
    dagtypid BIGINT,
    dagår BIGINT,
    dagår_räkenskapsår BIGINT,
    datum TIMESTAMP,
    kvartal BIGINT,
    kvartal_räkenskapsår VARCHAR,
    kvartalnamn VARCHAR,
    måldatum TIMESTAMP,
    månad BIGINT,
    månad_räkenskapsår VARCHAR,
    månadnamn VARCHAR,
    månadnamn_kort VARCHAR,
    vecka BIGINT,
    veckodagssiffra BIGINT,
    år BIGINT,
    år_räkenskapsår VARCHAR,
    årmånad VARCHAR,
    årmånad_räkenskapsår VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_remissdatum (
    capiovecka BIGINT,
    dagmånad BIGINT,
    dagnamn VARCHAR,
    dagnamn_kort VARCHAR,
    dagtyp VARCHAR,
    dagtypid BIGINT,
    dagår BIGINT,
    datum TIMESTAMP,
    kvartal BIGINT,
    kvartal_räkenskapsår VARCHAR,
    kvartalnamn VARCHAR,
    månad BIGINT,
    månad_räkenskapsår VARCHAR,
    månadnamn VARCHAR,
    månadnamn_kort VARCHAR,
    remissdatum TIMESTAMP,
    vecka BIGINT,
    veckodagssiffra BIGINT,
    år BIGINT,
    år_räkenskapsår VARCHAR,
    årmånad VARCHAR,
    årmånad_räkenskapsår VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_signeringsdatum (
    capiovecka BIGINT,
    dagmånad BIGINT,
    dagnamn VARCHAR,
    dagnamn_kort VARCHAR,
    dagtyp VARCHAR,
    dagtypid BIGINT,
    dagår BIGINT,
    datum TIMESTAMP,
    kvartal BIGINT,
    kvartal_räkenskapsår VARCHAR,
    kvartalnamn VARCHAR,
    månad BIGINT,
    månad_räkenskapsår VARCHAR,
    månadnamn VARCHAR,
    månadnamn_kort VARCHAR,
    signeringsdatum TIMESTAMP,
    vecka BIGINT,
    veckodagssiffra BIGINT,
    år BIGINT,
    år_räkenskapsår VARCHAR,
    årmånad VARCHAR,
    årmånad_räkenskapsår VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_undersökningsslut (
    capiovecka BIGINT,
    dagmånad BIGINT,
    dagnamn VARCHAR,
    dagnamn_kort VARCHAR,
    dagtyp VARCHAR,
    dagtypid BIGINT,
    dagår BIGINT,
    datum TIMESTAMP,
    kvartal BIGINT,
    kvartal_räkenskapsår VARCHAR,
    kvartalnamn VARCHAR,
    månad BIGINT,
    månad_räkenskapsår VARCHAR,
    månadnamn VARCHAR,
    månadnamn_kort VARCHAR,
    undersökningsslut TIMESTAMP,
    vecka BIGINT,
    veckodag_dynamisk VARCHAR,
    veckodagssiffra BIGINT,
    veckodagsiffra,_dynamisk DOUBLE,
    år BIGINT,
    år_räkenskapsår VARCHAR,
    årmånad VARCHAR,
    årmånad_räkenskapsår VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_undersökningsstart (
    capiovecka BIGINT,
    dagmånad BIGINT,
    dagnamn VARCHAR,
    dagnamn_kort VARCHAR,
    dagtyp VARCHAR,
    dagtypid BIGINT,
    dagår BIGINT,
    datum TIMESTAMP,
    kvartal BIGINT,
    kvartal_räkenskapsår VARCHAR,
    kvartalnamn VARCHAR,
    månad BIGINT,
    månad_räkenskapsår VARCHAR,
    månadnamn VARCHAR,
    månadnamn_kort VARCHAR,
    undersökningsstart TIMESTAMP,
    vecka BIGINT,
    veckodagssiffra BIGINT,
    år BIGINT,
    år_räkenskapsår VARCHAR,
    årmånad VARCHAR,
    årmånad_räkenskapsår VARCHAR
);

CREATE TABLE IF NOT EXISTS carmae (
    beskrivning VARCHAR,
    kategori VARCHAR,
    källa VARCHAR,
    namn VARCHAR,
    namn_och_beskrivning VARCHAR,
    objektkod BIGINT,
    objektnyckel VARCHAR,
    objektschemakod VARCHAR,
    objekttyp VARCHAR,
    skapad TIMESTAMP,
    status VARCHAR
);


-- Calculated tables (see calculated_tables.sql)
