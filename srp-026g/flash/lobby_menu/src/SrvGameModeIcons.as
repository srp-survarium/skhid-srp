package
{
   import flash.display.Bitmap;
   import flash.display.MovieClip;

   // A match's mode icon on the loading screen (LobbyGameLoad.setMode: frame `ui.icon` + 1). The
   // movie's own three frames (1 the protective device, 2 a transmitter mast, 3 Team Deathmatch's
   // skull), then srp's: 4 Artifact Hunt (`ui.icon` 3, GameModeIconArtefactHunt), where the
   // protective device's 61 x 55 lies, in its place (frame 1's art hidden under it).
   public dynamic class SrvGameModeIcons extends MovieClip
   {
      public static const ARTEFACT_HUNT:int = 4;

      private var artefactHunt:Bitmap;

      public function SrvGameModeIcons()
      {
         super();
      }

      override public function gotoAndStop(frame:Object, scene:String = null) : void
      {
         var own:Boolean = int(frame) == ARTEFACT_HUNT;
         super.gotoAndStop(own ? 1 : frame,scene);
         for(var i:int = 0; i < numChildren; i++)
         {
            var child:* = getChildAt(i);
            if(child != this.artefactHunt)
            {
               child.visible = !own;
            }
         }
         if(own && this.artefactHunt == null)
         {
            this.artefactHunt = new Bitmap(new GameModeIconArtefactHunt(),"auto",true);
            addChild(this.artefactHunt);
         }
         if(this.artefactHunt != null)
         {
            this.artefactHunt.visible = own;
         }
      }
   }
}
