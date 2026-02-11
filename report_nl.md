# 12UL next-gen timing- en trackingsysteem

### Een nieuwe teloplossing voor potentieel gebruik in volgende edities van 12Urenloop.

## Waarom?
- Het huidige systeem is geschikt om rondjes te tellen, maar niet in realtime (vertraging van max 30 seconden). Dit zorgt voor verwarring en wantrouw bij loopteams.
- De tracking op de site is een goeie inschatting, maar niet erg accuraat.
- De exacte duur van elke ronde is niet gekend.

## wat?
- Het huidige systeem is gebaseerd op bluetooth, het nieuwe systeem op UWB (Ultrawide Band, dezelfde techologie gebruikt om de afstand en richting van airtags tot je iphone te bepalen).
- Net zoals het huidige systeem werkt het via een aantal vaste stations en dezelfde batons met lichte electronica die de lopers dragen.

## Voordelen
- De exacte positie van lopers op het circuit meten met een precisie van ~50cm in realtime weten (met updates tenminste 1x per seconde).
- De rondetijden per team op de seconde meten.
- Stabielere metingen.
- De snelheid van elke loper op elk moment berekenen.

## Plan

Het doel is om bij de volgende editie van 12Urenloop een kleine test/demonstratie te geven van dit nieuw systeem. Liefst met 2 batons die naast het bestaand systeem ook het nieuwe systeem bezitten. De officiële telling zal nog steeds via het oude systeem werken, die kunnen we dan vergelijken met de data van het nieuwe systeem.


# Vooruitgang

- [x] Aankoop van 5 UWB modules (~150 euro inclusief BTW), nu eigendom van Zeus WPI.
- [x] Afstandsmetingen tussen 2 modules met een snelheid van 100+ metingen per seconde en een meetfout van ~15cm.
- [x] Afstandsmetingen tussen 2 modules met een snelheid van 600+ metingen per seconde, meetfout nog te bepalen maar rond of minder dan 50cm.
- [x] Draadloze timingdistributie tussen stations (nodig om hoge meetsnelheid te behalen) met een betrouwbaarheid van minder dan 0,0005 seconden.

# Roadmap

- [ ] Al deze elementen combineren in een volwaardig positioneringsysteem
- [ ] Software schrijven voor gebruik als een tel, tracking en timingsysteem
- [ ] Betrouwbaarheidstesting

# Materiaal

### Extra hardware nodig voor een baton:
- [DWM3000 module](https://www.digikey.com/en/products/detail/qorvo/DWM3000TR13/24367995): ~€20
    - Dit zal waarschijnlijk veranderd worden naar een [DWM3001C module](https://www.digikey.be/nl/products/detail/qorvo/DWM3001CSR/25862594): ~€25
- Misschien nog een kleine zelf ontworpen printplaat


### Extra hardware nodig voor een station:
- [DWM3000EVB module](https://www.digikey.be/nl/products/detail/qorvo/DWM3000EVB/24367408?s=N4IgTCBcDaICIHUCyBmADBgogNQEIgF0BfIA): ~€25
- [Wemos D1 R32 ESP32 microcontroller](https://www.otronic.nl/nl/wemos-d1-r32-esp32-4mb-development-board-wifi-blue.html): Te vinden voor minder dan 3 euro.

