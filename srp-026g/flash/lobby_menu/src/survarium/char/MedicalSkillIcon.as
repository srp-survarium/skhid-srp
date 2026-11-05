package survarium.char
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.PixelSnapping;
   import flash.display.Sprite;
   import MedicalSkill069301;
   import MedicalSkill069302;
   import MedicalSkill069311;
   import MedicalSkill069312;
   import MedicalSkill069321;
   import MedicalSkill069322;
   import MedicalSkill069331;
   import MedicalSkill069332;
   import MedicalSkill069341;
   import MedicalSkill069342;

   // Original 0.69d survival icons, copied unchanged; icons-069.tsv records their provenance.
   public class MedicalSkillIcon extends Sprite
   {
      private static const images:Object = {};
      private var image:Bitmap;

      public function MedicalSkillIcon()
      {
         mouseEnabled = false;
         mouseChildren = false;
         image = new Bitmap(null,PixelSnapping.AUTO,true);
         image.x = image.y = -17;
         addChild(image);
      }

      public function show(skill:uint) : void
      {
         var type:Class;
         switch(skill)
         {
            case 301: type = MedicalSkill069301; break;
            case 302: type = MedicalSkill069302; break;
            case 311: type = MedicalSkill069311; break;
            case 312: type = MedicalSkill069312; break;
            case 321: type = MedicalSkill069321; break;
            case 322: type = MedicalSkill069322; break;
            case 331: type = MedicalSkill069331; break;
            case 332: type = MedicalSkill069332; break;
            case 341: type = MedicalSkill069341; break;
            case 342: type = MedicalSkill069342; break;
            default: image.bitmapData = null; return;
         }
         if(images[skill] == null) images[skill] = new type() as BitmapData;
         image.bitmapData = images[skill];
         // Same 34-unit box as the renderer's scale calculation, with original aspect ratio.
         image.width = image.height = 34;
      }
   }
}
