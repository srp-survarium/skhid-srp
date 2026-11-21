package
{
   import flash.display.BitmapData;

   // The faces' window's portrait of face 9 (stk2_neutral_01_head; face-choice.md, FaceWindow): the lobby's own
   // character with that face, a three-quarter view in the Basement, a crop of a game capture at twice
   // the card's size (bitmaps/, a JPEG, which the build adds).
   public dynamic class SrpFace9 extends BitmapData
   {

      public function SrpFace9(w:int = 280, h:int = 360)
      {
         super(w,h);
      }
   }
}
