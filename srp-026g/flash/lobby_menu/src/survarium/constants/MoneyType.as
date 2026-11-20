package survarium.constants
{
   // Which money pays (the gold flag of a buy, craft or reroll request, which the exe still sends): always
   // silver, there is no gold (the owner, 2026-11-08).
   public class MoneyType
   {
      
      // silver (generic money)
      public static const REGULAR:int = 0;
      
      public function MoneyType()
      {
         super();
      }
   }
}
