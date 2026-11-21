package
{
   import flash.display.BitmapData;

   // The faces' window's portrait of face 0 (base_character_head; face-choice.md, FaceWindow): the lobby's own
   // character with that face, a three-quarter view in the Basement, a crop of a game capture at twice
   // the card's size (bitmaps/, a JPEG, which the build adds).
   public dynamic class SrpFace0 extends BitmapData
   {

      public function SrpFace0(w:int = 280, h:int = 360)
      {
         super(w,h);
      }
   }
}
