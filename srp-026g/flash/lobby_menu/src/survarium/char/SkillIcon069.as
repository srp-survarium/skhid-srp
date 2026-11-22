package survarium.char
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.PixelSnapping;
   import flash.display.Sprite;
   import ArtefactSkill069501;
   import ArtefactSkill069502;
   import ArtefactSkill069512;
   import ArtefactSkill069513;
   import ArtefactSkill069521;
   import ArtefactSkill069522;
   import ArtefactSkill069523;
   import ArtefactSkill069531;
   import ArtefactSkill069532;
   import ArtefactSkill069533;
   import ArtefactSkill069534;
   import ArtefactSkill069541;
   import ArtefactSkill069542;
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
   import TechnicalSkill069401;
   import TechnicalSkill069402;
   import TechnicalSkill069403;
   import TechnicalSkill069404;
   import TechnicalSkill069411;
   import TechnicalSkill069412;
   import TechnicalSkill069413;
   import TechnicalSkill069414;
   import TechnicalSkill069422;
   import TechnicalSkill069423;
   import TechnicalSkill069431;
   import TechnicalSkill069432;
   import TechnicalSkill069433;
   import TechnicalSkill069441;
   import TechnicalSkill069442;
   import TechnicalSkill069443;
   import TechnicalSkill069444;

   // Original 0.69d skill icons of our trees (medical: survival, Take Cover physical; technical:
   // engineering, utility; artefact: anomaly), copied unchanged;
   // scripts/{medical,technical,artefact}-skills/icons-069.tsv record their provenance.
   public class SkillIcon069 extends Sprite
   {
      private static const images:Object = {};
      private var image:Bitmap;

      public function SkillIcon069()
      {
         mouseEnabled = false;
         mouseChildren = false;
         image = new Bitmap(null,PixelSnapping.AUTO,true);
         image.x = image.y = -17;
         addChild(image);
      }

      // Shows the skill's icon; false for a skill of the stock trees, which has none here.
      public function show(skill:uint) : Boolean
      {
         var type:Class;
         switch(skill)
         {
            case 301: type = MedicalSkill069301; break;
            case 302: type = MedicalSkill069302; break;
            case 303: type = MedicalSkill069301; break;
            case 311: type = MedicalSkill069311; break;
            case 312: type = MedicalSkill069312; break;
            case 314: type = MedicalSkill069322; break;
            case 321: type = MedicalSkill069321; break;
            case 322: type = MedicalSkill069322; break;
            case 323: type = MedicalSkill069301; break;
            case 324: type = MedicalSkill069322; break;
            case 331: type = MedicalSkill069331; break;
            case 332: type = MedicalSkill069332; break;
            case 333: type = MedicalSkill069322; break;
            case 341: type = MedicalSkill069341; break;
            case 342: type = MedicalSkill069342; break;
            case 401: type = TechnicalSkill069401; break;
            case 402: type = TechnicalSkill069402; break;
            case 403: type = TechnicalSkill069403; break;
            case 404: type = TechnicalSkill069404; break;
            case 411: type = TechnicalSkill069411; break;
            case 412: type = TechnicalSkill069412; break;
            case 413: type = TechnicalSkill069413; break;
            case 414: type = TechnicalSkill069414; break;
            case 422: type = TechnicalSkill069422; break;
            case 423: type = TechnicalSkill069423; break;
            case 431: type = TechnicalSkill069431; break;
            case 432: type = TechnicalSkill069432; break;
            case 433: type = TechnicalSkill069433; break;
            case 441: type = TechnicalSkill069441; break;
            case 442: type = TechnicalSkill069442; break;
            case 443: type = TechnicalSkill069443; break;
            case 444: type = TechnicalSkill069444; break;
            case 501: type = ArtefactSkill069501; break;
            case 502: type = ArtefactSkill069502; break;
            case 512: type = ArtefactSkill069512; break;
            case 513: type = ArtefactSkill069513; break;
            case 521: type = ArtefactSkill069521; break;
            case 522: type = ArtefactSkill069522; break;
            case 523: type = ArtefactSkill069523; break;
            case 531: type = ArtefactSkill069531; break;
            case 532: type = ArtefactSkill069532; break;
            case 533: type = ArtefactSkill069533; break;
            case 534: type = ArtefactSkill069534; break;
            case 541: type = ArtefactSkill069541; break;
            case 542: type = ArtefactSkill069542; break;
            default: image.bitmapData = null; return false;
         }
         if(images[skill] == null) images[skill] = new type() as BitmapData;
         image.bitmapData = images[skill];
         // Same 34-unit box as the renderer's scale calculation, with original aspect ratio.
         image.width = image.height = 34;
         return true;
      }
   }
}
