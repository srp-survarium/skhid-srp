package
{
   import survarium.player.LocalPlayerProfile;
   
   public dynamic class SrvLocalPlayerProfile extends LocalPlayerProfile
   {
      
      public function SrvLocalPlayerProfile()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         glassSprite.mouseEnabled = false;
      }
   }
}

