package
{
   import survarium.ammo.AmmoRenderer;
   
   public dynamic class ResAmmo extends AmmoRenderer
   {
      
      public function ResAmmo()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         stop();
      }
   }
}

