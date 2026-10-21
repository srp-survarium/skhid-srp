package survarium.constants
{
   // An item renderer's look (ItemRendererCore.updateState).
   public class RendererState
   {
      
      // selected or hovered
      public static const ACTIVE:int = 0;
      
      public static const NORMAL:int = 1;
      
      // cannot be bought, under repair or greyed
      public static const LOCKED:int = 2;
      
      public function RendererState()
      {
         super();
      }
   }
}
