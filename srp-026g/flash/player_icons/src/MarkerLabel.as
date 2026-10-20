// Compile with the native optimized profile.
package
{
   import survarium.controls.SrvLabel;
   
   public dynamic class MarkerLabel extends SrvLabel
   {
      
      public function MarkerLabel()
      {
         super();
         addFrameScript(0,this.frame1,3,this.frame4,7,this.frame8,11,this.frame12,21,this.frame22);
      }
      
      internal function frame1() : *
      {
      }
      
      internal function frame4() : *
      {
         stop();
      }
      
      internal function frame8() : *
      {
         stop();
      }
      
      internal function frame12() : *
      {
         stop();
      }
      
      internal function frame22() : *
      {
         stop();
      }
   }
}

