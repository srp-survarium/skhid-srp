package survarium.controls
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   
   // The Sponge's combat-log icon (ui_desc/combat_log_icon 17 of artefact_sponge.options, frame 18
   // of the icon clip). 0.26g's movie has no picture in that frame; this is 0.28d0's (its hud.swf,
   // DefineBitsLossless2 491, 29 x 21), where 0.28d0's frame puts it: 2 px down.
   public class SpongeLogIcon extends Bitmap
   {
      
      public static const ICON:int = 17;
      
      private static const WIDTH:int = 29;
      
      private static const HEIGHT:int = 21;
      
      // A row a string, a pixel eight hex digits: ARGB, straight alpha.
      private static const PIXELS:Array = [
         "000000000000000000000000000000000000000000000000000000000000000051f9f9f9e1fdfdfdfdfdfdfdccfcfcfc68fafafa12e3e3e3000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
         "0000000000000000000000000000000000000000000000000000000007b6b6b6d7fdfdfdfffffffffffffffffffffffffffffffffbfdfdfdd2fdfdfdbcfcfcfcb2fcfcfc9bfcfcfc68fafafa22f0f0f0000000000000000000000000000000000000000000000000000000000000000000000000",
         "00000000000000000000000000000000000000000000000007b6b6b6a7fcfcfcfffffffff5fbfbfbe1e9e9e9fffffffffffffffffffffffffcfdfdfd8bfbfbfb4df8f8f8d5fdfdfdffffffffffffffffc8fcfcfc59f9f9f903555555000000000000000000000000000000000000000000000000",
         "000000000000000000000000000000000000000046f8f8f8d9fdfdfdffffffffadfbfbfb622c2c2c67000000c6cacacaffffffffffffffff90787878550000004c0000009ebebebefffffffffffffffff0fcfcfcbcfcfcfcb6fcfcfc39f6f6f60000000000000000000000000000000000000000",
         "00000000000000000000000008bfbfbf94fcfcfcffffffffffffffffe0fdfdfd471616166700000061000000a1b3b3b3fffffffffefbfbfb8a6b6b6b6500000066919191e7f9f9f9ffffffffffffffff7d49494951000000271a1a1a66f8f8f86ffafafa02000000000000000000000000000000",
         "00000000000000000fddddddbffcfcfcffffffffffffffffffffffffeef1f1f19d9191918cafafafa2f7f7f7fbfdfdfdc9e6e6e66e1a1a1aafadadaddef7f7f7fffdfdfdbafcfcfc7ebebebec8ccccccbabcbcbc600000004c0000004900000053c8c8c89bfcfcfc020000000000000000000000",
         "000000001becececd0fdfdfdfffffffff9fdfdfd7ca9a9a990787878ebeeeeeeffffffffe8fdfdfdb6fcfcfcfdfdfdfd7fc1c1c167000000680a0a0af0fcfcfcedfdfdfd190000005c00000080505050ffffffffa6f3f3f314e6e6e60200000042707070ffffffff83fbfbfb0000000000000000",
         "74fbfbfbf3fdfdfdffffffffffffffff92a9a9a96700000067000000bec1c1c1ebf0f0f03247474728000000acd4d4d4eef1f1f17178787836e3e3e3f5fbfbfbffffffff8968686850161616a8e5e5e5e5f4f4f4a3d5d5d5e0fdfdfdabfcfcfcd7fdfdfdfffffffffefcfcfc39f6f6f600000000",
         "f9fdfdfdffffffffffffffffffffffff7c4848486500000039707070ecfafafadbe5e5e5670000005d000000c3dcdcdcffffffffffffffffffffffffcbfcfcfc90878787ccd7d7d7f7fdfdfdffffffff5adadada670000006d888888fdfdfdfdfffffffffffffffffaf9f9f9c1e7e7e700000000",
         "edfdfdfdffffffffffffffffffffffffd4d9d9d98ad4d4d4e4fdfdfdffffffffffffffffd4e1e1e1d1fbfbfbffffffffe5f4f4f4e4ebebebffffffff4ac4c4c4610000006a0c0c0ce1f9f9f9ffffffff657e7e7e5900000038000000b1c0c0c0ffffffffabbcbcbc68000000b5b7b7b717000000",
         "a5fcfcfcffffffffffffffffffffffffffffffffe6fcfcfcd5fdfdfdffffffffffffffffffffffffcbfcfcfc3cf6f6f64500000068050505dfe4e4e4b5b6b6b65200000020000000c9e2e2e2ebeeeeee6800000020000000000000003fa6a6a6f6f6f6f66a0c0c0c670000009589898947000000",
         "16e8e8e8e5ebebebffffffffffffffffcfd5d5d54b0707072e000000acb8b8b8ffffffffffffffff85999999540000006700000059000000c2d4d4d4fcfbfbfb9fececec94fcfcfcfdfdfdfdfffcfcfc9f949494181515151fefefefa0fcfcfccbebebeb670000003400000046f8f8f834000000",
         "00000000533d3d3de8eaeaeaffffffff83585858670000005c000000939c9c9cffffffffffffffffc5cacaca520000001f3a3a3a5efafafaf7fdfdfdfffffffffffffffffffdfdfd99fcfcfc75858585d0ddddddffffffffffffffffffffffffbbf5f5f5600000000880808059f9f9f921000000",
         "0000000009000000a0afafafffffffffc3c7c7c74c17171706808080c2fcfcfcffffffffffffffffffffffffecfafafaf9fdfdfdd0fdfdfdabfcfcfcecf4f4f4ffffffffb5dcdcdc3c000000660000006f222222f2fbfbfbffffffffffffffffd9ededed4c00000081fbfbfb27f2f2f21d000000",
         "00000000000000004de5e5e5ffffffffffffffffe5fbfbfbe1fdfdfdfffffffff2fbfbfbedf0f0f0ffffffffffffffff8bfbfbfb050000002600000069131313b9bababac9cfcfcf670000003e00000024474747eff5f5f5fffffffffffffffffcfbfbfb9ebebebe60fafafa0200000025000000",
         "0000000000000000000000008bcdcdcdfcfbfbfbffffffffc7fbfbfb10bfbfbf4e00000067000000d2d7d7d7ffffffff826c6c6c6400000054000000450000005c0000009e939393d0e9e9e9a0fcfcfce1fdfdfdfffffffffffffffffffcfcfcccd2d2d26b565656000000003400000010000000",
         "0000000000000000000000001d0000007c484848c0c3c3c3f6f8f8f8908f8f8f80505050a6a9a9a9fcfbfbfbffffffffadaaaaaa38000000000000000000000005000000360000008eb4b4b4fffffffffffffffffcfdfdfdc5d2d2d2815555556700000064000000310000002600000000000000",
         "000000000000000000000000000000002e000000650000006e1a1a1a8896969690f8f8f8c2fcfcfcf2fdfdfdfffffffff7f7f7f766d7d7d70acccccc000000000355555522f0f0f093dcdcdcfcfbfbfbb2d7d7d7714a4a4a670000006700000066000000450000000f0000000000000000000000",
         "00000000000000000000000000000000000000000f0000003c0000006100000021000000120000002e0000007a4747479d919191aae0e0e0d0fdfdfdeafdfdfdfcfdfdfdd3fdfdfd81fbfbfb3e676767670000006700000065000000420000001200000000000000000000000000000000000000",
         "000000000000000000000000000000000000000000000000000000000500000021000000380000004e000000610000006700000061000000240000000500000000000000020000000e00000034000000640000003d0000000f000000000000000000000000000000000000000000000000000000",
         "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000f000000250000003d000000540000005d0000006500000055000000340000000b0000000000000000000000000000000000000000000000000000000000000000000000"];
      
      private static var picture:BitmapData;
      
      public function SpongeLogIcon()
      {
         super(SpongeLogIcon.draw());
         y = 2;
      }
      
      private static function draw() : BitmapData
      {
         var row:int;
         var column:int;
         var line:String;
         if(picture == null)
         {
            picture = new BitmapData(WIDTH,HEIGHT,true,0);
            for(row = 0; row < HEIGHT; row++)
            {
               line = PIXELS[row];
               for(column = 0; column < WIDTH; column++)
               {
                  picture.setPixel32(column,row,uint(parseInt(line.substr(column * 8,8),16)));
               }
            }
         }
         return picture;
      }
   }
}
