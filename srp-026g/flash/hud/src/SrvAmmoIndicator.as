package
{
   import survarium.ammo.AmmoIndicator;
   
   public dynamic class SrvAmmoIndicator extends AmmoIndicator
   {
      
      public function SrvAmmoIndicator()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         ammoAmmount.defaultTextFormat = ammoAmmount.getTextFormat();
      }
   }
}

