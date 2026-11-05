package
{
   import flash.display.BitmapData;

   // The match's squad icon 5 (the letter E in a ring): frame 5 of the hud's squad_icon
   // (PlayerListRenderer's, sprite 518 of the hud movie, the Tab list's squad mark), exported
   // with FFDec at 64 x 64 (bitmaps/, which the build adds). Tinted where it is drawn, as the
   // hud tints it (survarium.rooms.RoomsLook.squadIcon).
   public dynamic class SquadIcon5 extends BitmapData
   {
      public function SquadIcon5(w:int = 64, h:int = 64)
      {
         super(w,h);
      }
   }
}
