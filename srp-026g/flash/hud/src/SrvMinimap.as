package
{
   import survarium.controls.Minimap;
   
   public dynamic class SrvMinimap extends Minimap
   {
      
      public function SrvMinimap()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
      }
   }
}

