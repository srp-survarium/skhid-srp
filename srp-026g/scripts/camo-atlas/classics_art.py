#!/usr/bin/env python3
"""Draw the tiles of classics/ and artists/ (768 x 768 WebP) from free swatches and textures.

python3 classics_art.py fetch DIR                 download the sources below into DIR
python3 classics_art.py build --sources DIR [--out DIR] [--artists DIR]

classics/: real camouflage patterns. Each row crops a square of a free swatch, mostly from
Wikimedia Commons (SWATCHES: the file, its licence and the exact bytes used, by sha256), evening
out a photograph's lighting where asked, and makes it periodic with a minimum-cost seam on the
wrapped edges; a swatch that is itself a seamless tile is used whole (a row with no crop).
artists/: camouflage textures drawn by artists (ARTIST_ROWS), seamless tiles used whole or cropped
the same way. classics/SOURCES.txt and artists/SOURCES.txt say where each one comes from.
A source may be a member of an archive ('name::member'): a ZIP, or a 7z read with the 7z tool.
One swatch replaces an older tile of initial/ and is written there (INITIAL), keeping its item.
Pillow, NumPy and SciPy (and 7z for one archive); with NumPy 2.4.4, Pillow 13.0.0 and SciPy 1.17.1
it reproduces classics/*.webp, artists/*.webp and initial/mm-14.webp byte for byte. Each row's id
is its item in the game dictionary.
"""
import argparse
import csv
import hashlib
import io
import subprocess
import time
import zipfile
import urllib.request
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.ndimage import binary_dilation, distance_transform_edt, gaussian_filter, median_filter

HERE = Path(__file__).resolve().parent
SIZE = 768
COMMONS = 'https://commons.wikimedia.org/wiki/File:'

# name: (Commons file, download (the original or a standard thumbnail), its sha256, licence, author)
SWATCHES = {
    'lizard': ('French_Lizard_Pattern_Swatch_from_the_1950s.png',
               'https://thumb.wikimedia.org/wikipedia/commons/thumb/c/c0/French_Lizard_Pattern_Swatch_from_the_1950s.png/3840px-French_Lizard_Pattern_Swatch_from_the_1950s.png',
               '509994ed27ced6b3a775cc415f0ee8c298809698f1e444cbd7b57c81eafba31d', 'CC0', 'VoltigeurFR'),
    'erdl': ('Brown-dominant_%22highland%22_ERDL_camouflage_pattern_swatch.svg',
             'https://thumb.wikimedia.org/wikipedia/commons/thumb/a/a0/Brown-dominant_%22highland%22_ERDL_camouflage_pattern_swatch.svg/3840px-Brown-dominant_%22highland%22_ERDL_camouflage_pattern_swatch.svg.png',
             '81c7eb807ce57aa500241f26e2cefd4fe35163fbe8e1df4a111622e902a69fae', 'public domain (U.S. Government work)',
             'Illegitimate Barrister'),
    'dcu': ('DCU_camo_swatch.png',
            'https://upload.wikimedia.org/wikipedia/commons/8/82/DCU_camo_swatch.png',
            'f992a0dc395116e626d7ea214296ffd61870872e0ab17eec153055813caadf4c', 'public domain (U.S. Government work)',
            'Illegitimate Barrister'),
    'brush': ('Phase_I_All-Over_Brush_Urban_Camo_Close-Up_Swatch.png',
              'https://thumb.wikimedia.org/wikipedia/commons/thumb/6/6c/Phase_I_All-Over_Brush_Urban_Camo_Close-Up_Swatch.png/1280px-Phase_I_All-Over_Brush_Urban_Camo_Close-Up_Swatch.png',
              'a79a871defdae8dfe603529c3781a6dddb8b7d0def8071a8243a982b60aa5e2b', 'public domain (U.S. Government work)',
              'SergeantSelfExplanatory'),
    'cce': ('Bariolage_Centre-Europe.png',
            'https://upload.wikimedia.org/wikipedia/commons/1/1a/Bariolage_Centre-Europe.png',
            '793ca83c12eec6ac8308fd077a446fe9fd4f118d14eef779ea40074d89c04902', 'CC0', 'Youri BRIAND'),
    'bme': ('Bariolage_multi-environnement_(B.M.E.).png',
            'https://upload.wikimedia.org/wikipedia/commons/d/df/Bariolage_multi-environnement_%28B.M.E.%29.png',
            '10dfc0408264ab3ba89398f047bf3235517145404df93d7fbeb6bdc1a38cfd0e', 'CC0', 'Youri BRIAND'),
    'hu2015': ('2015M.png',
               'https://upload.wikimedia.org/wikipedia/commons/4/4e/2015M.png',
               '371316112e21753557e804e750af2631a64720c2540c6c9bc97b7a03dbe067f4', 'CC0', 'Shotya'),
    'mm14': ('MM14.jpg',
             'https://upload.wikimedia.org/wikipedia/commons/c/cd/MM14.jpg',
             '2355623bc74b08cf21ba632c9003dcc5b1488a3a39bedb045a227633ded42017', 'CC BY-SA 4.0',
             'Lucas Cavalcante Mesquita'),
    'rhodesian': ('Rhodesian_Brushstroke_pattern.jpg',
                  'https://upload.wikimedia.org/wikipedia/commons/1/19/Rhodesian_Brushstroke_pattern.jpg',
                  '4b1657794e12f6fe9c810bde0e38fcdea09a8ca4d847f058808b55ecc88b277d', 'CC BY-SA 3.0', 'Katangais'),
    'tropentarn': ('Tropentarn_Fabric_(cropped_2).jpg',
                   'https://thumb.wikimedia.org/wikipedia/commons/thumb/2/2d/Tropentarn_Fabric_%28cropped_2%29.jpg/1920px-Tropentarn_Fabric_%28cropped_2%29.jpg',
                   'c754c6d27774b12d7b3c1e5b0fe3d16e5661cdb25f6ab67a1aca013d922e8c2d', 'CC BY-SA 4.0', 'Murchison-Eye'),
    'bgdpm': ('Bulgarian_M2003_DPM_camouflage.jpg',
              'https://thumb.wikimedia.org/wikipedia/commons/thumb/c/c3/Bulgarian_M2003_DPM_camouflage.jpg/3840px-Bulgarian_M2003_DPM_camouflage.jpg',
              '8e9eee6572ab15f4eb0eb5b851c23c74971cd5ec87c5a6d0e2d7e0271b72ca74', 'CC BY-SA 4.0', 'Spektov'),
    'bgsplinter': ('Bulgarian_Splinter_camouflage.jpg',
                   'https://thumb.wikimedia.org/wikipedia/commons/thumb/2/2a/Bulgarian_Splinter_camouflage.jpg/3840px-Bulgarian_Splinter_camouflage.jpg',
                   '8c5393f2ffe5d562c49ff7363311c10452982f29402c5170616261269dc308ee', 'CC BY-SA 4.0', 'Spektov'),
    'nfpblue': ('M19_NFP_Blue_-_Dutch_pattern.jpg',
                'https://upload.wikimedia.org/wikipedia/commons/9/9a/M19_NFP_Blue_-_Dutch_pattern.jpg',
                '67d2731157410be98125e41fe9161fc16f78008131bbe1e013f62c6fff74e3c8', 'CC BY-SA 4.0',
                'Ministerie van Defensie (Netherlands)'),
    # The right-hand disc of the Ministry's explanation graphic of 4 Aug 2025 (initial/mm-25.webp).
    'mm25': ('https://mod.gov.ua/en/news/mo-d-clarifies-in-response-to-service-members-requests-trial-batches-of-clothing-and-gear-featuring-a-new-camouflage-pattern-will-be-procured',
             'https://mod.gov.ua/assets/34-p-camo-04aug-ws01-eng-704d2c5d84.webp',
             '237f648dc5ccad8c79f8e1e25d8eaaef30d6d1d7967fedf767ac1446b94f2275',
             'Ukrainian Ministry of Defence, 4 Aug 2025', 'Ministry of Defence of Ukraine'),
    'pantera': ('Kamuflaż_wz_93.png',
               'https://thumb.wikimedia.org/wikipedia/commons/thumb/1/1e/Kamufla%C5%BC_wz_93.png/1280px-Kamufla%C5%BC_wz_93.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
               'c0b6bbb9f4a91afcbdbee44315d702f63b242ef92f285afd2e581203c740104c', 'CC BY-SA 4.0', 'Magnum045'),
    'pantera-desert': ('Kamuflaż_wz._93_pustynny.png',
                      'https://thumb.wikimedia.org/wikipedia/commons/thumb/7/7f/Kamufla%C5%BC_wz._93_pustynny.png/1280px-Kamufla%C5%BC_wz._93_pustynny.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                      'cb2f7ddca34dfed6d67168c84e7ad278c152a8414d258cab32b6e0882b2c6c53', 'CC BY-SA 4.0', 'Magnum045'),
    'puma': ('Kamuflaż_wz._89.png',
            'https://thumb.wikimedia.org/wikipedia/commons/thumb/3/30/Kamufla%C5%BC_wz._89.png/1920px-Kamufla%C5%BC_wz._89.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
            'e6b05afe0a19306a2035ae81c9e71cce147e4ce7509a9efd42c537c98e4fa1c3', 'CC BY-SA 4.0', 'Magnum045'),
    'moro': ('Moro.wzor.tkaniny.svg',
            'https://thumb.wikimedia.org/wikipedia/commons/thumb/4/4a/Moro.wzor.tkaniny.svg/1920px-Moro.wzor.tkaniny.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
            '0122a287ec6892f401cbb07a37dc042503a65fca22d759bd517a0b4391ebbded', 'public domain (released by the author)', 'Julo'),
    'm-84': ('M84camo.jpeg',
            'https://thumb.wikimedia.org/wikipedia/commons/thumb/6/69/M84camo.jpeg/1280px-M84camo.jpeg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
            '352652dee1286d82a350bb9a1ce0a5b4a8e230766ece28fa891ee513c6944b21', 'CC BY-SA 4.0', 'Skjoldbro'),
    'm-01-desert': ('M01DKcamo.jpeg',
                   'https://thumb.wikimedia.org/wikipedia/commons/thumb/4/48/M01DKcamo.jpeg/1280px-M01DKcamo.jpeg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                   '775e725b8083b0ef6d10eefe6469e1c656862759d8312a20038965f41efa1c18', 'CC BY-SA 4.0', 'Skjoldbro'),
    'duck-hunter': ('Duckhunter.JPG',
                   'https://thumb.wikimedia.org/wikipedia/commons/thumb/1/18/Duckhunter.JPG/1920px-Duckhunter.JPG?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                   '599cd3b61b8f72af659f1da42a3ad95bacd6793d0285bc2916c28be180734178', 'CC BY-SA 3.0', 'gravedigger88'),
    'frogskin': ('Frog_Skin_camouflage_pattern.jpg',
                'https://upload.wikimedia.org/wikipedia/commons/2/21/Frog_Skin_camouflage_pattern.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original',
                'f71e9986c14e14a808b2ca995e8d3ef7ee47b1af04ca9dfd8c1c2ed128a35f90', 'CC BY-SA 3.0', 'IQ125'),
    'taz-83': ('Alpenflage_Taz_83_Tarnanzug_(cropped).jpg',
              'https://thumb.wikimedia.org/wikipedia/commons/thumb/4/44/Alpenflage_Taz_83_Tarnanzug_%28cropped%29.jpg/1920px-Alpenflage_Taz_83_Tarnanzug_%28cropped%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
              '1e073a4753fde872387eb759cd40841943815d8d4c0a39ce629a6d8f60ecd0d9', 'CC BY-SA 4.0', 'Qirille'),
    'taz-83-ii': ('Alpenflage_(cropped).JPG',
                 'https://thumb.wikimedia.org/wikipedia/commons/thumb/6/64/Alpenflage_%28cropped%29.JPG/1280px-Alpenflage_%28cropped%29.JPG?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                 '69f77cec64e10ab054dbe9649fd6b1c645a4a247f106019424cdbb9a8c0383e8', 'CC BY-SA 3.0', 'Bouterolle'),
    'arid-lizard': ('Greek_Arid_Lizard_M24.png',
                   'https://thumb.wikimedia.org/wikipedia/commons/thumb/7/73/Greek_Arid_Lizard_M24.png/960px-Greek_Arid_Lizard_M24.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                   'b210e37a9c22dc6cb74e128b47f334b3358d882fb4ec396c6cfef7ee7b4bf4ac', 'CC BY-SA 4.0', 'Hellenic Ministry of National Defence'),
    'sandstone': ('Cypriot_Sandstone.png',
                 'https://thumb.wikimedia.org/wikipedia/commons/thumb/a/ad/Cypriot_Sandstone.png/1280px-Cypriot_Sandstone.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                 '0a83919e1c20f1ff9ff38cf416b338c883ced75745c8f712649a526451a60f77', 'CC BY-SA 4.0', 'Onemanband01'),
    'vegetato-desert': ('Vegetato_deserto.jpg',
                       'https://thumb.wikimedia.org/wikipedia/commons/thumb/0/0d/Vegetato_deserto.jpg/1920px-Vegetato_deserto.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                       'c4cf821cca2eb9db0fb0e69d0b32caf60bb8bc6b5a25362b9dc18742a04a2e5e', 'public domain (released by the author)', 'Menelicche'),
    'dubok': ('Butan_Dubok.jpg',
             'https://thumb.wikimedia.org/wikipedia/commons/thumb/9/9d/Butan_Dubok.jpg/1920px-Butan_Dubok.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
             '247fafa04ea4caced8918bc795ec864cba40ed60f44231b77d664de27316ff81', 'CC BY-SA 4.0', 'GTRus'),
    'butan': ('Transnistrian_butan.png',
             'https://upload.wikimedia.org/wikipedia/commons/a/af/Transnistrian_butan.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original',
             '53bcb463390e548478bf85885967bb14c83c411f779bed88e44b10103d7323bb', 'CC BY-SA 4.0', 'JeSys'),
    'jgsdf-type-ii': ('Japanese_Flecktarn.png',
                     'https://thumb.wikimedia.org/wikipedia/commons/thumb/b/bb/Japanese_Flecktarn.png/1280px-Japanese_Flecktarn.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                     '95947b8b8a9cf4947acc1ab2595cbfd4dadd5e16772a702ee3babaaae6fd8884', 'CC BY-SA 4.0', 'Skjoldbro'),
    'jgsdf-desert': ('Japanese_Desert_Flecktarn.png',
                    'https://thumb.wikimedia.org/wikipedia/commons/thumb/2/2f/Japanese_Desert_Flecktarn.png/1280px-Japanese_Desert_Flecktarn.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                    '3889e91d8e95afcb8d963bc7b6c75ab48e285a7a90cf5125610bc4953c574aa8', 'CC BY-SA 4.0', 'Skjoldbro'),
    'jgsdf-winter': ('Japanese_Winter_Flecktarn.png',
                    'https://thumb.wikimedia.org/wikipedia/commons/thumb/c/c9/Japanese_Winter_Flecktarn.png/1280px-Japanese_Winter_Flecktarn.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                    'aeb0050590b0a25becb03ae3f8b65f77dbd2f11c237f78eab5549c6b1ef9ea16', 'CC BY-SA 4.0', 'Skjoldbro'),
    'tropentarn-ii': ('Tropentarn.png',
                     'https://thumb.wikimedia.org/wikipedia/commons/thumb/6/63/Tropentarn.png/1280px-Tropentarn.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                     '78c98aaef6cc84fcd73f898bdf03f2e4b30514928499aefbe50f4144b37a0748', 'CC BY-SA 4.0', 'Skjoldbro'),
    'urban-flecktarn': ('Polish_Urban_Flecktarn.png',
                       'https://thumb.wikimedia.org/wikipedia/commons/thumb/5/56/Polish_Urban_Flecktarn.png/1280px-Polish_Urban_Flecktarn.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                       '05795cbd13054aa48a04d86c4919fdd0a0310179f4d13455650a3b5a68257210', 'CC BY-SA 4.0', 'Skjoldbro'),
    'rust-flecktarn': ('Chinese_Flecktarn.png',
                      'https://thumb.wikimedia.org/wikipedia/commons/thumb/f/fb/Chinese_Flecktarn.png/1280px-Chinese_Flecktarn.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                      'd82646e73c3e587b64db854bc96048cf8459b91b626588c747fab2f4b10e320d', 'CC BY-SA 4.0', 'Skjoldbro'),
    'savanna-flecktarn': ('Indian_Flecktarn.png',
                         'https://thumb.wikimedia.org/wikipedia/commons/thumb/4/4e/Indian_Flecktarn.png/1280px-Indian_Flecktarn.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                         '8477b3ee0d9270cffb2106a7b148b6984c6da1b17d1e141b7c72e188ea5bf3f6', 'CC BY-SA 4.0', 'Skjoldbro'),
    'autumn-flecktarn': ('Belgian_Flecktarn.png',
                        'https://thumb.wikimedia.org/wikipedia/commons/thumb/3/3b/Belgian_Flecktarn.png/1280px-Belgian_Flecktarn.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                        'e80c7d119da6afd0cf0e4df1cbe10c81e38866e691de04cb85d2c82859793ffc', 'CC BY-SA 4.0', 'Skjoldbro'),
    't-99-winter': ('T99_DK_camo.jpg',
                   'https://thumb.wikimedia.org/wikipedia/commons/thumb/1/12/T99_DK_camo.jpg/1280px-T99_DK_camo.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                   'edcf2529c1a993864b35b0f67bb20216e3151f604c7dc8c051ab7a41f231a491', 'CC BY-SA 4.0', 'Skjoldbro'),
    'nfp-green': ('M19_NFP_Green_-_Dutch_pattern.jpg',
                 'https://thumb.wikimedia.org/wikipedia/commons/thumb/7/7a/M19_NFP_Green_-_Dutch_pattern.jpg/960px-M19_NFP_Green_-_Dutch_pattern.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                 '6426d499e832672fa21199196f6be8a177675eda6995dfdb3a733631ced8c296', 'CC BY-SA 4.0', 'Ministerie van Defensie (Netherlands)'),
    'nfp-multitone': ('M19_NFP_Multitone_-_Dutch_pattern.jpg',
                     'https://thumb.wikimedia.org/wikipedia/commons/thumb/7/76/M19_NFP_Multitone_-_Dutch_pattern.jpg/960px-M19_NFP_Multitone_-_Dutch_pattern.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                     'ad3df6b190c98eb68216884f213850a863ca81d94971202bd4bb91110c930107', 'CC BY-SA 4.0', 'Ministerie van Defensie (Netherlands)'),
    'nfp-cloth': ('Netherlands_Fractal_Pattern.jpg',
                 'https://thumb.wikimedia.org/wikipedia/commons/thumb/2/2c/Netherlands_Fractal_Pattern.jpg/1920px-Netherlands_Fractal_Pattern.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                 'c832a9e3c79a65c5e91b933a4a866e503856b57385b2871e19c2f1549c0a3de0', 'CC BY-SA 4.0', 'Ministerie van Defensie (Netherlands)'),
    'm90': ('Sweden_M90_pattern.svg',
           'https://thumb.wikimedia.org/wikipedia/commons/thumb/2/23/Sweden_M90_pattern.svg/1920px-Sweden_M90_pattern.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
           '27c6bd1f0ce0364f9c41fba41585eeb97ebc4e3b6b5f044ddd84538b9bc3119b', 'CC BY-SA 3.0', 'Henrickson at en.wikipedia'),
    'm90k': ('M90K_Camo.jpg',
            'https://thumb.wikimedia.org/wikipedia/commons/thumb/1/1c/M90K_Camo.jpg/1280px-M90K_Camo.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
            '9dab199c672e2dfd1e386c9812e9e8e929b175928524d2de959d703ae07732ca', 'CC BY-SA 4.0', 'Willarnold13'),
    'm81-woodland': ('US_Woodland_pattern.svg',
                    'https://thumb.wikimedia.org/wikipedia/commons/thumb/7/79/US_Woodland_pattern.svg/1920px-US_Woodland_pattern.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                    '98eda5a99c52252e09f21d6e6402c9980f5da6ba00383e78de277d7fbed43771', 'CC BY-SA 3.0', 'Henrickson at en.wikipedia'),
    'erdl-lowland': ('Green-dominant_"lowland"_ERDL_camouflage_pattern_swatch.svg',
                    'https://thumb.wikimedia.org/wikipedia/commons/thumb/9/9e/Green-dominant_%22lowland%22_ERDL_camouflage_pattern_swatch.svg/1920px-Green-dominant_%22lowland%22_ERDL_camouflage_pattern_swatch.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                    '0c77ad2e71d7a7b4cc39cc3fd885d9c66d225a4b5c5797d898a876451c90874f', 'public domain (U.S. Government work)', 'Illegitimate Barrister'),
    'daguet': ('Bariolage_Daguet_(Couleurs_CIELAB_officielles).png',
              'https://thumb.wikimedia.org/wikipedia/commons/thumb/2/20/Bariolage_Daguet_%28Couleurs_CIELAB_officielles%29.png/960px-Bariolage_Daguet_%28Couleurs_CIELAB_officielles%29.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
              '389abf6b2e6cd68ea468e5a0e5f7ad904d23adbe3192eb8dce52ac9344857178', 'CC0', 'Youri BRIAND'),
    'snow-terrain': ('Bariolage_zone_enneigée_(version_enneigée_du_Bariolage_multi-environnement).png',
                    'https://thumb.wikimedia.org/wikipedia/commons/thumb/7/7a/Bariolage_zone_enneig%C3%A9e_%28version_enneig%C3%A9e_du_Bariolage_multi-environnement%29.png/1920px-Bariolage_zone_enneig%C3%A9e_%28version_enneig%C3%A9e_du_Bariolage_multi-environnement%29.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                    '23689041a93c01fcae6173d1a03575803e020a3edc79c0b7eec62bf21827f5d0', 'CC0', 'Youri BRIAND'),
    'centre-europe-vehicle': ('Bariolage_Centre-Europe_(Variante_pour_véhicules).png',
                             'https://thumb.wikimedia.org/wikipedia/commons/thumb/9/9c/Bariolage_Centre-Europe_%28Variante_pour_v%C3%A9hicules%29.png/1920px-Bariolage_Centre-Europe_%28Variante_pour_v%C3%A9hicules%29.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                             'f793cdad814246867097e133b59d845bc30679c7a8c79dc2522ddef7b9fd1c64', 'CC0', 'Youri BRIAND'),
    'k20lq': ('K20LQ.png',
             'https://thumb.wikimedia.org/wikipedia/commons/thumb/3/34/K20LQ.png/1920px-K20LQ.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
             '0adb665b8bbe969cbedda65c5d0d0d1a044f819c9750711455224a087c88741e', 'CC BY-SA 4.0', 'Hieunguyen.e4'),
    'taz-07': ('TAZ_07.png',
              'https://thumb.wikimedia.org/wikipedia/commons/thumb/6/63/TAZ_07.png/1920px-TAZ_07.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
              'f944deb48fb95ca10ac132bcb2750e936c95cb9ca27b4bcd0b643d569d8f04b5', 'CC BY-SA 4.0', 'Fabrice Ram'),
    'dg3': ('USASOC_DG3_Pattern.svg',
           'https://thumb.wikimedia.org/wikipedia/commons/thumb/c/c4/USASOC_DG3_Pattern.svg/1920px-USASOC_DG3_Pattern.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
           '7d9c93d250c3b01bb775e0839544f155f7cc2b375b00a380217bc982b6036200', 'CC BY-SA 4.0', 'Savageearthllc'),
    'fab-lizard': ('Camo_FAB.jpg',
                  'https://upload.wikimedia.org/wikipedia/commons/5/56/Camo_FAB.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original',
                  'e7212db3caf49222a2038958b5df36fcb4008ccd6dc6643e9ff1a58a318d84af', 'CC BY-SA 3.0', 'Guilmann'),
    'm05-woodland': ('M05_woodland_pattern.jpg',
                    'https://upload.wikimedia.org/wikipedia/commons/3/30/M05_woodland_pattern.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original',
                    '6c1459c4929f7e4e032cf0c71c95e46f8053f9b24790003b80064b5cc66ec975', 'public domain (released by the author)', 'Pekka Vilhunen'),
    'm05-snow': ('M05_snow_pattern.jpg',
                'https://upload.wikimedia.org/wikipedia/commons/0/05/M05_snow_pattern.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original',
                '613f0888046c12056f089fab8e6596826b176b41d238e44665dd053520148773', 'public domain (released by the author)', 'Pekka Vilhunen'),
    'lozenge': ('Lozenge_4.png',
               'https://upload.wikimedia.org/wikipedia/commons/7/77/Lozenge_4.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original',
               '929d0e47d111d95553c05e39f10da3a25c5b154ba807ee52f44c20020b139066', 'CC0', 'unknown author (via flight-depot.com)'),
    'lozenge-night': ('Lozenge_5.png',
                     'https://upload.wikimedia.org/wikipedia/commons/7/7d/Lozenge_5.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original',
                     'e9d7c138d95fae6354e351da64a231ad4a3a633d13cfa3741532711ea703e686', 'CC0', 'unknown author (via flight-depot.com)'),
    'buntfarben-1918': ('Buntfarbenmuster_1918.svg',
                       'https://thumb.wikimedia.org/wikipedia/commons/thumb/4/47/Buntfarbenmuster_1918.svg/1920px-Buntfarbenmuster_1918.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                       '89ccfe3d0f2421feff66fcf5e6f5715d2096fe7aabefb0b6cd0f93887e6cf45f', 'public domain (released by the author)', 'Mediatus'),
    'obh': ('Tanzug_OBH_multienvironment.png',
           'https://thumb.wikimedia.org/wikipedia/commons/thumb/d/d3/Tanzug_OBH_multienvironment.png/1280px-Tanzug_OBH_multienvironment.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
           '5b5d798c3aa6de4af89dce1d26123a07ea987beaaca17b33c5ee501b202161be', 'CC0', 'Fabrice Ram'),
    'blue-tiger-stripe': ('USAF_Blue_Tigerstripe_pattern_closeup.jpg',
                         'https://thumb.wikimedia.org/wikipedia/commons/thumb/b/b7/USAF_Blue_Tigerstripe_pattern_closeup.jpg/1920px-USAF_Blue_Tigerstripe_pattern_closeup.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                         '1bdd2ec320aa649470da334d8c2cb2abda2908ae7852e2eae52726223c271c73', 'public domain (U.S. Government work)', 'U.S. Air Force'),
    'rmp-woodland': ('RMP_Subdued_Woodland_pattern.jpg',
                    'https://thumb.wikimedia.org/wikipedia/commons/thumb/0/0f/RMP_Subdued_Woodland_pattern.jpg/1920px-RMP_Subdued_Woodland_pattern.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                    '2a48ee36ca466bb0dc1e9e6b1856559e63a18eadc7489da423075c96697743f4', 'CC0', 'Rizuan'),
    'type-07-universal': ('Type_07_universal.jpg',
                         'https://upload.wikimedia.org/wikipedia/commons/8/87/Type_07_universal.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original',
                         'bb094ea1081085d4b93d9bb92e370c4a4c05ce63582be7726903f2cb7abc6841', 'CC BY-SA 4.0', 'GTRus'),
    'type-07-arid': ('Type_07_arid.jpg',
                    'https://thumb.wikimedia.org/wikipedia/commons/thumb/e/e8/Type_07_arid.jpg/960px-Type_07_arid.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                    '2f1ee6dce1c530684c9ec25cce049a56cbf211320f17ee26b11023d6ba10f884', 'CC BY-SA 4.0', 'GTRus'),
    'type-07-urban': ('Type_07_urban.jpg',
                     'https://thumb.wikimedia.org/wikipedia/commons/thumb/6/60/Type_07_urban.jpg/960px-Type_07_urban.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                     '5e99ea029f0886153e1ce2ea17a809f7a63acf70bd29a9048f4a345c7d2e3670', 'CC BY-SA 4.0', 'GTRus'),
    'k07': ("People's_Army_of_Vietnam_-_K07_(Type_07)_camouflage_-_Ground_Force.jpg",
           'https://upload.wikimedia.org/wikipedia/commons/d/dd/People%27s_Army_of_Vietnam_-_K07_%28Type_07%29_camouflage_-_Ground_Force.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original',
           '73c829f4d1652700daac05bbbd0c8dd6c4b8fd768be48e175b732d7a53690ab2', 'CC BY-SA 4.0', 'Hủ Tíu Mỳ Khô'),
    'k07-dark': ("People's_Army_of_Vietnam_K07_Ground_Force_Camoflauge_Vector_Version.svg",
                'https://thumb.wikimedia.org/wikipedia/commons/thumb/6/6d/People%27s_Army_of_Vietnam_K07_Ground_Force_Camoflauge_Vector_Version.svg/1920px-People%27s_Army_of_Vietnam_K07_Ground_Force_Camoflauge_Vector_Version.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                '5bcbb0d9baec30e3360983f07f660e8d37b1e23edf5f68cc0a7e19774681c61f', 'CC BY 4.0', 'LeenoDT'),
    'k20': ("People's_Army_of_Vietnam_K20_Ground_Force_Camouflage_Vector_Version.svg",
           'https://thumb.wikimedia.org/wikipedia/commons/thumb/c/cd/People%27s_Army_of_Vietnam_K20_Ground_Force_Camouflage_Vector_Version.svg/1920px-People%27s_Army_of_Vietnam_K20_Ground_Force_Camouflage_Vector_Version.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
           '6ca1e29d9b44a10bebea78cff9ca22ac456246819387a515071b9e168a4a8c69', 'CC BY 4.0', 'LeenoDT'),
    'k20-navy': ("People's_Army_of_Vietnam_K20_Navy_Camouflage_Vector_Version.svg",
                'https://thumb.wikimedia.org/wikipedia/commons/thumb/3/38/People%27s_Army_of_Vietnam_K20_Navy_Camouflage_Vector_Version.svg/1920px-People%27s_Army_of_Vietnam_K20_Navy_Camouflage_Vector_Version.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                'ca2d69dd522f7f54efdc619c01b697aea479f13ea16e1ee5b83d43065ee27465', 'CC BY 4.0', 'LeenoDT'),
    'vehicle-pixel-2015': ('Ukrainian_military_vehicle_pixel_camouflage,_2015_pattern.svg',
                          'https://thumb.wikimedia.org/wikipedia/commons/thumb/f/f5/Ukrainian_military_vehicle_pixel_camouflage%2C_2015_pattern.svg/1920px-Ukrainian_military_vehicle_pixel_camouflage%2C_2015_pattern.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                          '8197f1fe96058a517a10984719191c3c02af81c7b32bce229b856e642be80149', 'CC BY-SA 4.0', 'VileGecko'),
    'vehicle-pixel-2016': ('Ukrainian_military_vehicle_pixel_camouflage,_2016_pattern.svg',
                          'https://thumb.wikimedia.org/wikipedia/commons/thumb/5/53/Ukrainian_military_vehicle_pixel_camouflage%2C_2016_pattern.svg/1920px-Ukrainian_military_vehicle_pixel_camouflage%2C_2016_pattern.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                          '6515f105bc5b732395d3ee1b5b0e1bf9efe9a2ef5c7406a408756b37997d03b0', 'CC BY-SA 4.0', 'VileGecko'),
    'vehicle-pixel-2021': ('Ukrainian_military_vehicle_pixel_camouflage,_2021_pattern.svg',
                          'https://thumb.wikimedia.org/wikipedia/commons/thumb/f/f7/Ukrainian_military_vehicle_pixel_camouflage%2C_2021_pattern.svg/1920px-Ukrainian_military_vehicle_pixel_camouflage%2C_2021_pattern.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                          '468ac5a6006a1fdef25630848ddea8c7c571ed2cc1a57c22f380dc4d99b425ca', 'CC BY-SA 4.0', 'VileGecko'),
    'nobiax-31': ('https://opengameart.org/content/free-materials-pack-31-redux',
                 'https://opengameart.org/sites/default/files/3D_pattern_31.7z',
                 'c1aaf58c814246bb806e65aef907ce0a46374004697512dfdf169788b611e3d0', 'CC0', 'Nobiax (Yughues)'),
    'urban-blots': ('https://opengameart.org/content/camo-texture-pack-1',
                   'https://opengameart.org/sites/default/files/Untitled-1_5.png',
                   'a17bbc315ae4093ad33b780e4e5a48634d93b2076170d65da88a30b2f7516c3a', 'CC0', '6666'),
    'lime-spots': ('https://opengameart.org/content/camo-texture-pack-1',
                  'https://opengameart.org/sites/default/files/Untitled-2_0.png',
                  'e32ca88af002863d44b9ba465618cbe759e1bb5bef26e8219a804989458e1b04', 'CC0', '6666'),
    'blue-brush': ('https://opengameart.org/content/blue-camo-4k',
                  'https://opengameart.org/sites/default/files/blucamo4k.png',
                  'e8b4beeb8022727888b666e132ba767126fb07ccce04b6fcce339e292f64d260', 'CC BY 3.0', 'cron'),
    'woven-woodland': ('https://3dtextures.me/2019/07/30/fabric-camo-003/',
                      'https://drive.usercontent.google.com/download?export=download&id=1v5bDgRizyw8Mw_CgS8xztlHDlp4J21Hh',
                      '1f4bcb420dda2fb461e0f89d4a2fbb919711adf0df3bb109df47f6689bfbb24c', 'CC0', 'João Paulo (3dtextures.me)'),
    'urban-knit': ('https://3dtextures.me/2016/05/17/fabric-camo-001/',
                  'https://drive.usercontent.google.com/download?export=download&id=1zlfrTAaWWj8xGXb0qGAyy-fBGEJLw3Bv',
                  '59c695e70eb1b1238343ba8617231e05481be92abd5ad3b58370994c04af15ec', 'CC0', 'João Paulo (3dtextures.me)'),
    'star-forest': ('0_0_0_Exército_do_Lula_-_camuflagem_floresta.svg',
                   'https://thumb.wikimedia.org/wikipedia/commons/thumb/5/5d/0_0_0_Ex%C3%A9rcito_do_Lula_-_camuflagem_floresta.svg/1920px-0_0_0_Ex%C3%A9rcito_do_Lula_-_camuflagem_floresta.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                   'bda6816f32063381d1999e030390716db00c47e23ae1827c074d91505c614dd5', 'CC BY-SA 4.0', 'Eugenio Hansen, OFS'),
    'star-ember': ('0_0_0_Exército_do_Lula_-_camuflagem_bombeiros.svg',
                  'https://thumb.wikimedia.org/wikipedia/commons/thumb/d/de/0_0_0_Ex%C3%A9rcito_do_Lula_-_camuflagem_bombeiros.svg/1920px-0_0_0_Ex%C3%A9rcito_do_Lula_-_camuflagem_bombeiros.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                  '385c75cf9f44aff758b68e0957182ed501c02fb01cc967fe0a20a0a6f00248a4', 'CC BY-SA 4.0', 'Eugenio Hansen, OFS'),
    'marine-woodland': ('Armea_kamuflado_-_mararmea_1.svg',
                       'https://thumb.wikimedia.org/wikipedia/commons/thumb/9/98/Armea_kamuflado_-_mararmea_1.svg/1920px-Armea_kamuflado_-_mararmea_1.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                       '8ff49afdd992442c5c650f9e677cc1e0dd607c4fd4847958256334a406f5f831', 'CC BY-SA 4.0', 'Eugenio Hansen, OFS'),
    'urban-woodland': ('Armea_kamuflado_-_urba_2.svg',
                      'https://thumb.wikimedia.org/wikipedia/commons/thumb/f/ff/Armea_kamuflado_-_urba_2.svg/1920px-Armea_kamuflado_-_urba_2.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                      'ef91be61c50f0d4cfc580871ad4e528fe146f9402372ef2ab76511a00ac82f0b', 'CC BY-SA 4.0', 'Eugenio Hansen, OFS'),
    'hazard-pixel': ('Armea_kamuflado_-_UTFPR.svg',
                    'https://thumb.wikimedia.org/wikipedia/commons/thumb/2/2c/Armea_kamuflado_-_UTFPR.svg/1920px-Armea_kamuflado_-_UTFPR.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                    '75c087bfb67b30b985b10f9c45350f10eafd2592be6fd5c807e079c0d8130b0b', 'CC BY-SA 4.0', 'Eugenio Hansen, OFS, after Aleksey Belousov'),
    'teal-pixel': ('Armea_kamuflado_-_turkiza.svg',
                  'https://thumb.wikimedia.org/wikipedia/commons/thumb/5/57/Armea_kamuflado_-_turkiza.svg/1920px-Armea_kamuflado_-_turkiza.svg.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail',
                  '7ac196cfb0d2ae4527237ff49a842bb0ab68325dd0a9b802077ab56541b7a17b', 'CC BY-SA 4.0', 'Eugenio Hansen, OFS, after Aleksey Belousov'),
}


def seam(cost):
    """The top-to-bottom path of least summed cost through a rows x columns array."""
    rows, cols = cost.shape
    total = cost.copy()
    back = np.zeros((rows, cols), int)
    for r in range(1, rows):
        prev = total[r - 1]
        options = np.stack([np.r_[np.inf, prev[:-1]], prev, np.r_[prev[1:], np.inf]])
        step = options.argmin(0)
        back[r] = np.arange(cols) + step - 1
        total[r] += options.min(0)
    path = np.zeros(rows, int)
    path[-1] = total[-1].argmin()
    for r in range(rows - 1, 0, -1):
        path[r - 1] = back[r, path[r]]
    return path


def wrap_columns(crop, n):
    """rows x (n + o) -> rows x n whose last column continues into its first.

    The first o columns take the o columns past n left of a least-difference seam,
    so the tile's right edge meets the pixels that followed it in the swatch."""
    o = crop.shape[1] - n
    after, start = crop[:, n:], crop[:, :o]
    path = seam(np.abs(after.astype(np.int32) - start).sum(2).astype(np.float64))
    tile = crop[:, :n].copy()
    take = np.arange(o)[None, :] < path[:, None]
    tile[:, :o][take] = after[take]
    return tile


def periodic(crop, n):
    """(n + o) x (n + o) swatch crop -> n x n tile that wraps on both axes."""
    rows = wrap_columns(crop, n)
    return wrap_columns(rows.transpose(1, 0, 2), n).transpose(1, 0, 2)


def source_pixels(sources, ref):
    """The RGB pixels of a source ('name', or 'name::member' of an archive), its bytes checked."""
    name, _, member = ref.partition('::')
    file, _, digest = SWATCHES[name][:3]
    data = (sources / name).read_bytes()
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f'{sources / name}: not the expected {file}')
    if member and data[:2] == b'PK':
        data = zipfile.ZipFile(io.BytesIO(data)).read(member)
    elif member:
        data = subprocess.run(['7z', 'x', '-so', name, member], cwd=sources, check=True,
                              capture_output=True).stdout
    with Image.open(io.BytesIO(data)) as image:
        return np.asarray(image.convert('RGB'))


def clear_marks(pixels, boxes):
    """Paint out small printed marks: in each (x0, y0, x1, y1) box, the pixels that stand out from
    their neighbourhood (a 9 px median) take the colour of the nearest pixel that does not, softened."""
    out = pixels.copy()
    for x0, y0, x1, y1 in boxes:
        box = out[y0:y1, x0:x1].astype(np.float32)
        luma = box.mean(2)
        mask = binary_dilation(np.abs(luma - median_filter(luma, 9)) > 10, iterations=2)
        _, (rows, cols) = distance_transform_edt(mask, return_indices=True)
        soft = gaussian_filter(box[rows, cols], (1.5, 1.5, 0))
        out[y0:y1, x0:x1] = np.clip(np.rint(np.where(mask[:, :, None], soft, box)), 0, 255).astype(np.uint8)
    return out


def cut(sources, how):
    """The tile a row's swatch tuple describes; a trailing dict gives swatch()'s keyword options."""
    if isinstance(how[-1], dict):
        return swatch(sources, *how[:-1], **how[-1])
    return swatch(sources, *how)


def swatch(sources, name, x=None, y=None, n=None, overlap=None, repeats=False, light=0, *, marks=(), area=None):
    """marks: boxes of printed marks to paint out first; area: (x0, y0, x1, y1) of the swatch to
    keep before the lighting is evened out (x and y are then within it)."""
    pixels = source_pixels(sources, name)
    if marks:
        pixels = clear_marks(pixels, marks)
    if area:
        pixels = pixels[area[1]:area[3], area[0]:area[2]]
    if x is None:
        # A seamless tile: used whole (its largest square from the top left).
        side = min(pixels.shape[:2])
        return Image.fromarray(pixels[:side, :side]).resize((SIZE, SIZE), Image.Resampling.LANCZOS)
    if light:
        # A photograph: divide out its broad light and shade (a blur of light radius) at constant mean.
        values = pixels.astype(np.float32)
        shade = gaussian_filter(values.mean(2), light, mode='reflect')
        pixels = np.clip(np.rint(values * (shade.mean() / shade)[:, :, None]), 0, 255).astype(np.uint8)
    span = n + overlap
    if repeats:
        # The swatch is itself a repeat: the crop may run over its edges.
        crop = pixels.take(range(y, y + span), 0, mode='wrap').take(range(x, x + span), 1, mode='wrap')
    else:
        crop = pixels[y:y + span, x:x + span]
    if crop.shape[:2] != (span, span):
        raise ValueError(f'{name}: crop outside the swatch')
    return Image.fromarray(periodic(crop, n)).resize((SIZE, SIZE), Image.Resampling.LANCZOS)


# id (the item; 3107-3110 were drawn studies, dropped 2026-11-26), slug, name, family, description,
# and the swatch: (name, x, y, n, overlap[, repeats[, light]][, {keyword options}]), or (name,) for a
# seamless tile used whole
ROWS = [
    (3099, 'lizard', 'Lizard', 'Lizard',
     'Brushed horizontal strokes of green and brown over a pale ground, from a 1950s French swatch.',
     ('lizard', 1280, 0, 1500, 200)),
    (3100, 'erdl-highland', 'ERDL Highland', 'ERDL',
     'Brown-dominant leaf pattern with black branches, from the U.S. ERDL highland swatch.',
     ('erdl', 704, 0, 1850, 260)),
    (3101, 'three-color-desert', 'Three-Color Desert', 'Desert Tricolor',
     'Broad khaki clouds and thin brown ridges on sand, from the U.S. desert uniform swatch.',
     ('dcu', 256, 640, 700, 100, True)),
    (3102, 'urban-brush', 'Urban Brush', 'All-Over Brush',
     'Sweeping dry-brush marks in plum, rose and cream, from a 2002 U.S. Army trial swatch.',
     ('brush', 0, 0, 740, 100)),
    (3103, 'centre-europe', 'Centre Europe', 'Centre Europe',
     'Wide horizontal shapes in green, brown, sand and black, from the French CCE swatch.',
     ('cce', 384, 0, 1091, 128, True)),
    (3104, 'all-terrain', 'All Terrain', 'Multi-Environment',
     'Layered gradients of sand, olive and brown with small bright flecks, from the French BME swatch.',
     ('bme', 320, 0, 2250, 300)),
    (3105, 'danube-leaf', 'Danube Leaf', 'Leaf Mosaic',
     'Interlocking leaf shapes in ochre, olive and brown with pale veins, from the Hungarian 2015M swatch.',
     ('hu2015', 0, 0, 960, 140)),
    (3106, 'brushstroke', 'Brushstroke', 'Brushstroke',
     'Diagonal green and brown dry-brush strokes on sand, from a Rhodesian brushstroke swatch.',
     ('rhodesian', 0, 0, 600, 90)),
    (3111, 'tropentarn', 'Tropentarn', 'Desert Fleck',
     'Brown, green and sand flecks on a pale khaki ground, from a German Tropentarn fabric photograph.',
     ('tropentarn', 576, 0, 1150, 150, False, 150)),
    (3112, 'balkan-dpm', 'Balkan DPM', 'Disruptive Pattern',
     'Interlocking brush shapes in brown, olive, grey, navy and cream, from the Bulgarian M2003 DPM.',
     ('bgdpm', 1792, 768, 1800, 240, False, 150)),
    (3113, 'balkan-splinter', 'Balkan Splinter', 'Splinter Rain',
     'Grey and plum splinters over a beige ground with rain marks, from the Bulgarian splinter pattern.',
     ('bgsplinter', 1792, 704, 1800, 240)),
    (3114, 'nfp-blue', 'NFP Blue', 'Mottle',
     'Fine navy and slate mottling, from the Dutch M19 NFP Blue pattern.',
     ('nfpblue', 0, 0, 740, 90)),
    # Added 2026-11-26: the owner's picks of the real-pattern candidates.
    (3115, 'pantera', 'Pantera', 'Woodland',
     'Blotches of green, brown and black on olive, from the Polish wz. 93 Pantera drawing.',
     ('pantera', 96, 300, 998, 124)),
    (3116, 'pantera-desert', 'Pantera Desert', 'Woodland',
     'Khaki, brown and green blotches on sand, from the Polish desert wz. 93 Pantera drawing.',
     ('pantera-desert', 96, 300, 998, 124)),
    (3117, 'puma', 'Puma', 'Leopard Spot',
     'Small olive and brown cells over khaki, from the Polish wz. 89 Puma drawing.',
     ('puma', 126, 153, 1497, 187)),
    (3118, 'moro', 'Moro', 'Woodland',
     'Large black, brown and sand shapes on khaki, from a Polish Moro fabric drawing.',
     ('moro', 329, 98, 1123, 140)),
    (3119, 'm-84', 'M/84', 'Fleck',
     'Bright and dark green, brown and black flecks, from a Danish M/84 swatch.',
     ('m-84', 436, 56, 684, 85)),
    (3120, 'm-01-desert', 'M/01 Desert', 'Fleck',
     'Brown, khaki and coral flecks on sand, from a Danish M/01 desert swatch.',
     ('m-01-desert', 436, 52, 684, 85)),
    (3121, 'duck-hunter', 'Duck Hunter', 'Spot',
     'Rounded brown, olive and green spots on pale green, from a photograph of a U.S. WWII spot pattern.',
     ('duck-hunter', 329, 98, 1123, 140)),
    (3122, 'frogskin', 'Frogskin', 'Spot',
     'Large brown and green spots on pale green, from a photograph of the U.S. Marine Corps frogskin.',
     ('frogskin', 229, 23, 307, 38)),
    (3123, 'taz-83', 'TAZ 83', 'Alpine Dot',
     'Red, green, brown and black dabs on olive, from a photograph of the Swiss TAZ 83 Alpenflage.',
     ('taz-83', 684, 72, 1005, 125)),
    (3124, 'taz-83-ii', 'TAZ 83 II', 'Alpine Dot',
     'A second TAZ 83 swatch: the same red, green and brown dabs on a more worn cloth.',
     ('taz-83-ii', 404, 60, 715, 89)),
    (3125, 'arid-lizard', 'Arid Lizard', 'Lizard',
     'Brushed horizontal strokes of khaki, brown and green on sand, from the Greek M24 Arid Lizard drawing.',
     ('arid-lizard', 56, 64, 748, 93)),
    (3126, 'sandstone', 'Sandstone', 'Woodland',
     'Woodland shapes in sand, tan, brown and grey, from the Cypriot Sandstone drawing.',
     ('sandstone', 76, 52, 659, 82)),
    (3127, 'vegetato-desert', 'Vegetato Desert', 'Mottle',
     'Soft pink, grey and brown mottling, from a photograph of the Italian desert Vegetato.',
     ('vegetato-desert', 112, 120, 1303, 162)),
    (3128, 'dubok', 'Dubok', 'Woodland',
     'Dark teal, black and mint blotches, from a photograph of the Butan Dubok pattern.',
     ('dubok', 553, 84, 1123, 140)),
    (3129, 'butan', 'Butan', 'Woodland',
     'Green, ochre and brown blotches, from a drawing of the Butan pattern.',
     ('butan', 38, 28, 323, 40)),
    (3130, 'jgsdf-type-ii', 'JGSDF Type II', 'Fleck',
     'Green, brown and black flecks on khaki, from a drawing of the Japanese Type II pattern.',
     ('jgsdf-type-ii', 256, 52, 680, 85)),
    (3131, 'jgsdf-desert', 'JGSDF Desert', 'Fleck',
     'Brown and tan flecks on cream, from a drawing of the Japanese desert pattern.',
     ('jgsdf-desert', 348, 52, 680, 85)),
    (3132, 'jgsdf-winter', 'JGSDF Winter', 'Fleck',
     'Dark green, brown and black flecks on white, from a drawing of the Japanese winter pattern.',
     ('jgsdf-winter', 440, 60, 680, 85)),
    (3133, 'tropentarn-ii', 'Tropentarn II', 'Desert Fleck',
     'Brown and green flecks on pale sand, from a drawing of the German Tropentarn.',
     ('tropentarn-ii', 76, 52, 680, 85)),
    (3134, 'urban-flecktarn', 'Urban Flecktarn', 'Fleck',
     'Grey, black and white flecks, from a drawing of an urban flecktarn.',
     ('urban-flecktarn', 76, 52, 680, 85)),
    (3135, 'rust-flecktarn', 'Rust Flecktarn', 'Fleck',
     'Grey, black and rust-orange flecks on white, from a drawing of a flecktarn variant.',
     ('rust-flecktarn', 76, 52, 680, 85)),
    (3136, 'savanna-flecktarn', 'Savanna Flecktarn', 'Fleck',
     'Green, brown and tan flecks on khaki, from a drawing of a flecktarn variant.',
     ('savanna-flecktarn', 76, 52, 680, 85)),
    (3137, 'autumn-flecktarn', 'Autumn Flecktarn', 'Fleck',
     'Grey-green, orange-brown and black flecks, from a drawing of a flecktarn variant.',
     ('autumn-flecktarn', 76, 52, 680, 85)),
    (3138, 't-99-winter', 'T/99 Winter', 'Fleck',
     'Green and coral flecks on white, from a Danish T/99 winter swatch.',
     ('t-99-winter', 436, 60, 684, 85)),
    (3139, 'nfp-green', 'NFP Green', 'Mottle',
     'Fine green, brown and khaki mottling, from the Dutch M19 NFP Green pattern.',
     ('nfp-green', 129, 36, 499, 62)),
    (3140, 'nfp-multitone', 'NFP Multitone', 'Mottle',
     'Fine sand, khaki, green and brown mottling, from the Dutch M19 NFP Multitone pattern.',
     ('nfp-multitone', 273, 36, 499, 62)),
    (3141, 'nfp-cloth', 'NFP Cloth', 'Mottle',
     'Green and brown NFP mottling on folded cloth, from a photograph of Dutch NFP fabric.',
     ('nfp-cloth', 114, 84, 998, 124)),
    (3142, 'm90', 'M90', 'Splinter',
     'Hard-edged splinters of green, olive and dark violet, from a drawing of the Swedish M90.',
     ('m90', 108, 126, 1497, 187)),
    (3143, 'm90k', 'M90K', 'Splinter',
     'Hard-edged sand, pink and brown splinters, from a photograph of the Swedish desert M90K.',
     ('m90k', 380, 48, 643, 80)),
    (3144, 'm81-woodland', 'M81 Woodland', 'Woodland',
     'Green, brown, sand and black woodland shapes, from a drawing of the U.S. M81 Woodland.',
     ('m81-woodland', 144, 108, 1497, 187)),
    (3145, 'erdl-lowland', 'ERDL Lowland', 'ERDL',
     'Green-dominant leaf pattern with black branches, from the U.S. ERDL lowland swatch.',
     ('erdl-lowland', 490, 70, 836, 104)),
    (3146, 'daguet', 'Daguet', 'Centre Europe',
     'Wide horizontal shapes in sand, ochre and brown, from the French Daguet swatch.',
     ('daguet', 56, 36, 468, 58)),
    (3147, 'snow-terrain', 'Snow Terrain', 'Multi-Environment',
     'Layered white, pale grey and grey shapes, from the snow version of the French BME swatch.',
     ('snow-terrain', 890, 60, 815, 101)),
    (3148, 'centre-europe-vehicle', 'Centre Europe Vehicle', 'Centre Europe',
     'Very large green, brown and black shapes, from the vehicle version of the French CCE swatch.',
     ('centre-europe-vehicle', 144, 108, 1496, 187)),
    (3149, 'k20lq', 'K20LQ', 'Woodland',
     'Dark green, red-brown and teal woodland shapes, from the K20LQ drawing.',
     ('k20lq', 144, 711, 1497, 187)),
    (3150, 'taz-07', 'TAZ 07', 'Blotch',
     'Soft grey, pink and green blotches, from the TAZ 07 drawing.',
     ('taz-07', 192, 120, 1371, 171)),
    (3151, 'dg3', 'DG3', 'Digital Mottle',
     'Fine olive, brown and tan digital mottling, from the USASOC DG3 drawing.',
     ('dg3', 144, 135, 1497, 187)),
    (3152, 'fab-lizard', 'FAB Lizard', 'Lizard',
     'Blue-grey and green brush strokes on olive, from a photograph of Brazilian Air Force lizard fabric.',
     ('fab-lizard', 26, 74, 348, 43)),
    (3153, 'm05-woodland', 'M05 Woodland', 'Fractal Pixel',
     'Fine green, brown and black pixel clusters, from a photograph of the Finnish M05 woodland.',
     ('m05-woodland', 282, 48, 521, 65)),
    (3154, 'm05-snow', 'M05 Snow', 'Fractal Pixel',
     'Grey pixel clusters on white, from a photograph of the Finnish M05 snow pattern.',
     ('m05-snow', 54, 36, 491, 61)),
    (3155, 'lozenge', 'Lozenge', 'Lozenge',
     'Irregular hexagons of violet, green, blue and ochre, from a drawing of WWI German lozenge fabric.',
     ('lozenge', 24, 90, 337, 42)),
    (3156, 'lozenge-night', 'Lozenge Night', 'Lozenge',
     'Smaller hexagons in dark violet, blue, red and grey, from a drawing of WWI German night lozenge.',
     ('lozenge-night', 30, 34, 343, 42)),
    (3157, 'buntfarben-1918', 'Buntfarben 1918', 'Polygon',
     'Large ochre, brown and green polygons edged in black, from a drawing of the German 1918 scheme.',
     ('buntfarben-1918', 108, 108, 1497, 187)),
    (3158, 'obh', 'OBH', 'Fleck',
     'Blue-grey, green and brown flecks on khaki, from a drawing of the Hungarian OBH pattern.',
     ('obh', 248, 52, 699, 87)),
    (3159, 'blue-tiger-stripe', 'Blue Tiger Stripe', 'Tiger Stripe',
     'Navy, blue-grey and white tiger stripes, from a photograph of the U.S. Air Force blue tigerstripe.',
     ('blue-tiger-stripe', 275, 85, 936, 117)),
    (3160, 'rmp-woodland', 'RMP Woodland', 'Woodland',
     'Soft grey-green, olive and khaki woodland shapes, from a photograph of Malaysian RMP fabric.',
     ('rmp-woodland', 112, 98, 1123, 140)),
    (3161, 'type-07-universal', 'Type 07 Universal', 'Digital',
     'White, grey and green digital fleck, from a swatch of the Chinese Type 07 universal pattern.',
     ('type-07-universal', 60, 52, 680, 85)),
    (3162, 'type-07-arid', 'Type 07 Arid', 'Digital',
     'Brown, tan and dark digital fleck, from a swatch of the Chinese Type 07 arid pattern.',
     ('type-07-arid', 64, 56, 748, 93)),
    (3163, 'type-07-urban', 'Type 07 Urban', 'Digital',
     'Grey, black and khaki digital fleck, from a swatch of the Chinese Type 07 urban pattern.',
     ('type-07-urban', 64, 64, 748, 93)),
    (3164, 'k07', 'K07', 'Woodland',
     'Vivid green, red-brown and yellow woodland shapes, from a photograph of Vietnamese K07 fabric.',
     ('k07', 42, 54, 562, 70)),
    (3165, 'k07-dark', 'K07 Dark', 'Woodland',
     'Dark green, brown and khaki woodland shapes, from a vector drawing of the Vietnamese K07.',
     ('k07-dark',)),
    (3166, 'k20', 'K20', 'Brushstroke',
     'Green, khaki, navy and brown brush shapes, from a vector drawing of the Vietnamese K20.',
     ('k20',)),
    (3167, 'k20-navy', 'K20 Navy', 'Brushstroke',
     'Blue, pale blue and brown brush shapes, from a vector drawing of the Vietnamese K20 navy pattern.',
     ('k20-navy',)),
    (3168, 'vehicle-pixel-2015', 'Vehicle Pixel 2015', 'Digital',
     'Large olive, cream and black pixel blocks, from a drawing of the Ukrainian 2015 vehicle pattern.',
     ('vehicle-pixel-2015',)),
    (3169, 'vehicle-pixel-2016', 'Vehicle Pixel 2016', 'Digital',
     'Large olive, cream, black and brown pixel blocks, from a drawing of the Ukrainian 2016 vehicle pattern.',
     ('vehicle-pixel-2016',)),
    (3170, 'vehicle-pixel-2021', 'Vehicle Pixel 2021', 'Digital',
     'Large olive, sand, cream and grey pixel blocks, from a drawing of the Ukrainian 2021 vehicle pattern.',
     ('vehicle-pixel-2021',)),
]

# Rows of artists/catalog.tsv: camouflage textures drawn by artists (artists/SOURCES.txt).
ARTIST_ROWS = [
    (3171, 'shard-forest', 'Shard Forest', 'Shards',
     'Torn shards of dark green and brown with soft painted shading, by Nobiax.',
     ('nobiax-31::pattern_157/diffuse.tga',)),
    (3172, 'shard-winter', 'Shard Winter', 'Shards',
     'Torn shards of pale grey, slate and teal, by Nobiax.',
     ('nobiax-31::pattern_154/diffuse.tga',)),
    (3173, 'shard-earth', 'Shard Earth', 'Shards',
     'Torn shards of brown, ochre and grey, by Nobiax.',
     ('nobiax-31::pattern_158/diffuse.tga',)),
    (3174, 'urban-blots', 'Urban Blots', 'Digital Blob',
     'Crisp stepped blobs of black, white and two greys, by 6666.',
     ('urban-blots',)),
    (3175, 'lime-spots', 'Lime Spots', 'Spot',
     'Olive and dark spots on a pale lime ground, by 6666.',
     ('lime-spots',)),
    (3176, 'blue-brush', 'Blue Brush', 'Brushstroke',
     'Sharp navy and ice-blue brush shapes, by cron.',
     ('blue-brush',)),
    (3177, 'woven-woodland', 'Woven Woodland', 'Woodland',
     'High-contrast green, black, brown and khaki woodland printed on a visible weave, by João Paulo.',
     ('woven-woodland',)),
    (3178, 'urban-knit', 'Urban Knit', 'Woodland',
     'Grey and black blobs on a coarse knit, by João Paulo.',
     ('urban-knit',)),
    (3179, 'star-forest', 'Star Forest', 'Star Fleck',
     'Green and olive flecks shaped like small stars, by Eugenio Hansen.',
     ('star-forest', 318, 1272, 576, 72)),
    (3180, 'star-ember', 'Star Ember', 'Star Fleck',
     'Orange and red flecks shaped like small stars, by Eugenio Hansen.',
     ('star-ember', 318, 1272, 576, 72)),
    (3181, 'marine-woodland', 'Marine Woodland', 'Woodland',
     'Teal, navy and white woodland blobs with soft shading, by Eugenio Hansen.',
     ('marine-woodland',)),
    (3182, 'urban-woodland', 'Urban Woodland', 'Woodland',
     'Grey, black and brick-red woodland blobs with soft shading, by Eugenio Hansen.',
     ('urban-woodland',)),
    (3183, 'hazard-pixel', 'Hazard Pixel', 'Digital',
     'Black, yellow and white recolour of the Russian EMR digital pattern, by Eugenio Hansen.',
     ('hazard-pixel',)),
    (3184, 'teal-pixel', 'Teal Pixel', 'Digital',
     'Turquoise and dark teal recolour of the Russian EMR digital pattern, by Eugenio Hansen.',
     ('teal-pixel',)),
]

# Rows of initial/catalog.tsv whose tile is a swatch made here (it replaced a study under the same item).
INITIAL = [
    (2812, 'mm-14', ('mm14', 0, 192, 640, 96)),
    # The square inside the disc, left of a printed mark at its right edge; two other marks painted out.
    (2813, 'mm-25', ('mm25', 0, 0, 312, 64, False, 50,
                     {'marks': ((1428, 456, 1480, 490), (1320, 669, 1349, 693)), 'area': (1270, 354, 1646, 730)})),
]


def palette(image):
    quant = image.resize((128, 128), Image.Resampling.BOX).quantize(colors=5)
    colors = np.array(quant.getpalette(), dtype=np.uint8).reshape(-1, 3)
    return ' '.join('#' + bytes(colors[i]).hex().upper() for count, i in sorted(quant.getcolors(), reverse=True))


def origin(file):
    """The page a source is credited to, and the site's name."""
    if not file.startswith('https://'):
        return COMMONS + file, 'Wikimedia Commons'
    return file, {'opengameart.org': 'OpenGameArt', '3dtextures.me': '3dtextures.me',
                  'mod.gov.ua': 'Ministry of Defence of Ukraine'}[file.split('/')[2]]


def draw(sources, out, table):
    out.mkdir(parents=True, exist_ok=True)
    rows = []
    for item, slug, name, family, description, how in table:
        image = cut(sources, how)
        file, _, _, licence, author = SWATCHES[how[0].partition('::')[0]][:5]
        url, site = origin(file)
        image.save(out / (slug + '.webp'), quality=94, method=4)
        rows.append((item, slug, name, family, description, url, f'{site} · {author} · {licence}', palette(image)))
        print(slug, flush=True)
    with (out / 'catalog.tsv').open('w', encoding='utf-8', newline='') as stream:
        writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
        writer.writerow(('id', 'slug', 'name', 'family', 'description', 'source_url', 'source_label', 'palette'))
        writer.writerows(rows)


def build(sources, out, artists, initial):
    draw(sources, out, ROWS)
    draw(sources, artists, ARTIST_ROWS)
    for item, slug, how in INITIAL:
        image = cut(sources, how)
        image.save(initial / (slug + '.webp'), quality=94, method=4)
        print(f'{slug} (initial/, palette {palette(image)})', flush=True)


def fetch(target):
    target.mkdir(parents=True, exist_ok=True)
    for name, (_, url, digest, *_) in SWATCHES.items():
        path = target / name
        while not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            request = urllib.request.Request(url, headers={'User-Agent': 'camo-atlas-fetch/1.0'})
            try:
                path.write_bytes(urllib.request.urlopen(request, timeout=300).read())
            except OSError as error:
                print(f'{name}: {error}; retrying in a minute', flush=True)
                time.sleep(60)
        print(name, digest, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    get = commands.add_parser('fetch')
    get.add_argument('dir', type=Path)
    make = commands.add_parser('build')
    make.add_argument('--sources', required=True, type=Path)
    make.add_argument('--out', type=Path, default=HERE / 'classics')
    make.add_argument('--artists', type=Path, default=HERE / 'artists')
    make.add_argument('--initial', type=Path, default=HERE / 'initial')
    args = parser.parse_args()
    if args.command == 'fetch':
        fetch(args.dir)
    else:
        build(args.sources, args.out, args.artists, args.initial)


if __name__ == '__main__':
    main()
