package
{
   import survarium.buttons.ButtonNotification;
   
   public dynamic class SquadInviteButtonRenderer extends ButtonNotification
   {
      
      public function SquadInviteButtonRenderer()
      {
         super();
         addFrameScript(3,this.frame4,7,this.frame8,11,this.frame12,21,this.frame22);
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

